use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    env,
    path::PathBuf,
};

use anyhow::{Context, Result, bail};
use domain::{
    form::{
        answer::RedmineIssueId,
        question::QuestionType,
        redmine_import::{RedmineImportTarget, RedmineIssueRelation, RedmineIssueRelationBatch},
    },
    types::authorization_guard::{Allowed, Read},
};
use futures::{StreamExt, stream};
use redmine_importer::{
    Config, EXCLUDED_TRACKERS, InquiryClassification, RedmineApi, TARGET_TRACKERS,
    build_issue_input, classify_inquiry, unique_issue_relations,
};
use resource::{database::connection::RedmineImportConnectionPool, repository::Repository};
use usecase::redmine_import::{RedmineImportUseCase, prepare_issue, validate_question_value};

#[derive(Clone, Copy, Debug)]
enum Mode {
    Plan,
    Import,
    Verify,
}

impl Mode {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "plan" => Ok(Self::Plan),
            "import" => Ok(Self::Import),
            "verify" => Ok(Self::Verify),
            _ => bail!("モードは plan / import / verify のいずれかを指定してください"),
        }
    }
}

struct PreparedIssue {
    project_id: i64,
    tracker_name: String,
    issue_id: RedmineIssueId,
    inquiry_classification: InquiryClassification,
    issue: domain::form::redmine_import::RedmineImportedIssue,
}

#[derive(Default)]
struct Report {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl Report {
    fn error(&mut self, message: impl Into<String>) {
        self.errors.push(message.into());
    }

    fn warning(&mut self, message: impl Into<String>) {
        self.warnings.push(message.into());
    }

    fn print(&self) {
        for warning in &self.warnings {
            println!("WARNING: {warning}");
        }
        for error in &self.errors {
            println!("ERROR: {error}");
        }
        println!(
            "report: {} error(s), {} warning(s)",
            self.errors.len(),
            self.warnings.len()
        );
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    run().await
}

async fn run() -> Result<()> {
    let (mode, config_path) = parse_args()?;
    let config = Config::load(config_path)?;
    let api = RedmineApi::from_env(&config)?;

    let statuses = api.fetch_issue_statuses().await?;
    let mut report = Report::default();
    if let Err(error) = config.validate_statuses(&statuses) {
        report.error(error.to_string());
        report.print();
        bail!("status mapping が不完全なため移行を開始できません");
    }

    let repository = Repository::new(RedmineImportConnectionPool::new().await?);
    let usecase = RedmineImportUseCase::new(&repository);
    let mut target_cache: HashMap<
        (i64, Option<String>, InquiryClassification),
        Allowed<RedmineImportTarget, Read>,
    > = HashMap::new();

    for &(tracker_id, tracker_name) in TARGET_TRACKERS {
        let status_names = config
            .status_label_mappings
            .get(&tracker_id)
            .map(|status_labels| status_labels.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let target_variants = std::iter::once(None)
            .chain(status_names.into_iter().map(Some))
            .collect::<Vec<_>>();
        let classifications = if tracker_id == 4 {
            vec![
                InquiryClassification::Generic,
                InquiryClassification::Mod,
                InquiryClassification::Login,
                InquiryClassification::ExperienceOverflow,
                InquiryClassification::Appeal,
            ]
        } else {
            vec![InquiryClassification::Generic]
        };

        for status_name in target_variants {
            for classification in classifications.iter().copied() {
                let mapping =
                    config.form_mapping_for(tracker_id, classification.special_route())?;
                let form_id = match classification.special_route() {
                    Some(route) => config.form_id_for_inquiry(route)?,
                    None => config.form_id_for(tracker_id)?,
                };
                let status_label_name = status_name
                    .as_deref()
                    .and_then(|name| config.status_label_for(tracker_id, name))
                    .map(str::to_owned);
                let mut labels = mapping.labels.clone();
                if let Some(status_label_name) = &status_label_name {
                    labels.push(status_label_name.clone());
                }

                let target = usecase
                    .find_target(form_id, &mapping.form_title, &labels)
                    .await?
                    .with_context(|| {
                        let variant = status_name.as_deref().map_or_else(
                            || "通常状態".to_string(),
                            |name| format!("status={name:?}"),
                        );
                        format!(
                            "tracker {tracker_id} ({tracker_name:?}, {variant}, classification={classification:?}) の Portal form が見つかりません: id={}, title={:?}",
                            mapping.form_id, mapping.form_title
                        )
                    });
                let target = match target {
                    Ok(target) => target,
                    Err(error) => {
                        report.error(error.to_string());
                        continue;
                    }
                };
                let mut question_keys = mapping
                    .question_mappings
                    .keys()
                    .cloned()
                    .collect::<BTreeSet<_>>();
                question_keys.extend(mapping.custom_field_question_mappings.values().cloned());
                if let Some(question) = target.questions().iter().find(|question| {
                    question.is_required()
                        && !question_keys.contains(question.template_key().as_str())
                }) {
                    report.error(format!(
                        "tracker {tracker_id} ({tracker_name:?}, classification={classification:?}) の必須 question {:?} に mapping がありません",
                        question.template_key()
                    ));
                }

                if let Some(template_key) = mapping.question_mappings.keys().find(|template_key| {
                    !target
                        .questions()
                        .iter()
                        .any(|question| question.template_key().as_str() == template_key.as_str())
                }) {
                    report.error(format!(
                        "tracker {tracker_id} ({tracker_name:?}, classification={classification:?}) の question mapping がフォームに存在しません: {template_key:?}"
                    ));
                }

                let custom_field_content_question = target.questions().iter().find(|question| {
                    question.template_key().as_str()
                        == mapping.custom_field_content_template_key.as_str()
                });
                match custom_field_content_question {
                    None => report.error(format!(
                        "tracker {tracker_id} ({tracker_name:?}, classification={classification:?}) の custom field 本文 mapping がフォームに存在しません: {:?}",
                        mapping.custom_field_content_template_key
                    )),
                    Some(question) if question.question_type() != QuestionType::Text => {
                        report.error(format!(
                            "tracker {tracker_id} ({tracker_name:?}, classification={classification:?}) の custom field 本文 mapping は Text question でなければなりません: {:?}",
                            mapping.custom_field_content_template_key
                        ));
                    }
                    Some(_) => {}
                }

                for (field_id, template_key) in &mapping.custom_field_question_mappings {
                    match target.questions().iter().find(|question| {
                        question.template_key().as_str() == template_key.as_str()
                    }) {
                        None => report.error(format!(
                            "tracker {tracker_id} ({tracker_name:?}, classification={classification:?}) の custom field {field_id} mapping がフォームに存在しません: {template_key:?}"
                        )),
                        Some(question)
                            if !matches!(
                                question.question_type(),
                                QuestionType::Text
                                    | QuestionType::SingleChoice
                                    | QuestionType::MultipleChoice
                            ) =>
                        {
                            report.error(format!(
                                "tracker {tracker_id} ({tracker_name:?}, classification={classification:?}) の custom field {field_id} mapping の question type が不正です: {template_key:?}"
                            ));
                        }
                        Some(_) => {}
                    }
                }

                for (template_key, source) in &mapping.question_mappings {
                    let redmine_importer::QuestionValueSource::Static { value } = source else {
                        continue;
                    };
                    if let Err(error) =
                        validate_question_value(&target, template_key, value.clone())
                    {
                        report.error(format!(
                            "tracker {tracker_id} ({tracker_name:?}, classification={classification:?}) の static question mapping が不正です: {error}"
                        ));
                    }
                }
                target_cache.insert((tracker_id, status_name.clone(), classification), target);
            }
        }
    }
    if !report.errors.is_empty() {
        report.print();
        bail!("Portal form/question の事前検証に失敗しました。DB は変更していません");
    }

    let issue_stubs = api.fetch_issue_stubs(config.page_size).await?;
    let issue_relations = unique_issue_relations(&issue_stubs)?;
    let tracker_by_issue = issue_stubs
        .iter()
        .map(|stub| (stub.id, stub.tracker_id))
        .collect::<BTreeMap<_, _>>();
    let mut target_stubs = Vec::new();

    for stub in issue_stubs {
        if let Some((_, expected_name)) = EXCLUDED_TRACKERS
            .iter()
            .find(|(tracker_id, _)| *tracker_id == stub.tracker_id)
        {
            if stub.tracker_name != *expected_name {
                report.error(format!(
                    "issue {} の tracker ID {} の名前が一致しません: expected={expected_name:?}, actual={:?}",
                    stub.id, stub.tracker_id, stub.tracker_name
                ));
                continue;
            }
            report.warning(format!(
                "issue {} は対象外 tracker {:?} のため移行しません",
                stub.id, stub.tracker_name
            ));
            continue;
        }
        let Some((_, expected_name)) = TARGET_TRACKERS
            .iter()
            .find(|(tracker_id, _)| *tracker_id == stub.tracker_id)
        else {
            report.error(format!(
                "issue {} の tracker ID {} ({:?}) は対象にも対象外一覧にもありません",
                stub.id, stub.tracker_id, stub.tracker_name
            ));
            continue;
        };
        if stub.tracker_name != *expected_name {
            report.error(format!(
                "issue {} の tracker ID {} の名前が一致しません: expected={expected_name:?}, actual={:?}",
                stub.id, stub.tracker_id, stub.tracker_name
            ));
            continue;
        }

        let issue_id = match RedmineIssueId::try_new(stub.id) {
            Ok(issue_id) => issue_id,
            Err(error) => {
                report.error(format!("issue {} の ID が不正です: {error}", stub.id));
                continue;
            }
        };
        target_stubs.push((stub, issue_id));
    }

    let api_ref = &api;
    let fetched_issues = stream::iter(target_stubs.into_iter().map(|(stub, issue_id)| {
        let api = api_ref;
        async move {
            api.fetch_issue(issue_id)
                .await
                .map(|issue| (stub, issue_id, issue))
        }
    }))
    .buffer_unordered(config.detail_concurrency)
    .collect::<Vec<_>>()
    .await;
    let mut fetched_issues = fetched_issues.into_iter().collect::<Result<Vec<_>>>()?;
    fetched_issues.sort_by_key(|(stub, _, _)| stub.id);

    let mut prepared_issues = Vec::new();
    let mut inquiry_counts = BTreeMap::new();
    for (stub, issue_id, issue) in fetched_issues {
        let expected_project_name = config.project_name_for(stub.project_id)?;
        if issue.project.id != stub.project_id || issue.project.name != expected_project_name {
            report.error(format!(
                "issue {} の project が一覧と詳細または設定で一致しません: list={} {:?} / detail={} {:?} / expected={:?}",
                stub.id,
                stub.project_id,
                expected_project_name,
                issue.project.id,
                issue.project.name,
                expected_project_name
            ));
            continue;
        }
        if issue.tracker.id != stub.tracker_id || issue.tracker.name != stub.tracker_name {
            report.error(format!(
                "issue {} の tracker が一覧と詳細で一致しません: {} {:?} / {} {:?}",
                stub.id, stub.tracker_id, stub.tracker_name, issue.tracker.id, issue.tracker.name
            ));
            continue;
        }

        if let Some(attachments) = issue.attachments.as_ref()
            && !attachments.is_empty()
        {
            let filenames = attachments
                .iter()
                .map(|attachment| format!("{}({})", attachment.filename, attachment.id))
                .collect::<Vec<_>>()
                .join(", ");
            report.warning(format!(
                "issue {} の添付本体は移行しません: {filenames}",
                issue.id
            ));
        }
        let status = match config.status_for(&issue.status.name) {
            Ok(status) => status,
            Err(error) => {
                report.error(format!("issue {}: {error}", issue.id));
                continue;
            }
        };

        let status_target_key = config
            .status_label_for(stub.tracker_id, &issue.status.name)
            .map(|_| issue.status.name.clone());
        let inquiry_classification = if stub.tracker_id == 4 {
            classify_inquiry(&issue.subject, issue.description.as_deref())
        } else {
            InquiryClassification::Generic
        };
        let Some(target) =
            target_cache.get(&(stub.tracker_id, status_target_key, inquiry_classification))
        else {
            report.error(format!(
                "issue {} の status {:?} / classification {:?} に対応する Portal form/label target がありません",
                issue.id, issue.status.name, inquiry_classification
            ));
            continue;
        };

        let question_values_result = match inquiry_classification.special_route() {
            Some(route) => config.question_values_for_inquiry(route, &issue),
            None => config.question_values_for(stub.tracker_id, &issue),
        };
        let question_values = match question_values_result {
            Ok(question_values) => question_values,
            Err(error) => {
                report.error(format!("issue {}: {error}", issue.id));
                continue;
            }
        };
        let built = match build_issue_input(issue, status, question_values) {
            Ok(built) => built,
            Err(error) => {
                report.error(format!(
                    "issue {} の移行データを作れません: {error}",
                    stub.id
                ));
                continue;
            }
        };
        for journal_id in built.skipped_empty_journal_ids {
            report.warning(format!(
                "issue {} の journal {} は notes が空のためコメントとしては移行しません",
                stub.id, journal_id
            ));
        }
        let imported = match prepare_issue(
            target,
            built.input,
            redmine_importer::publication_for_tracker(stub.tracker_id),
        ) {
            Ok(imported) => imported,
            Err(error) => {
                report.error(format!(
                    "issue {} を Domain へ変換できません: {error}",
                    stub.id
                ));
                continue;
            }
        };
        prepared_issues.push(PreparedIssue {
            project_id: stub.project_id,
            tracker_name: stub.tracker_name,
            issue_id,
            inquiry_classification,
            issue: imported,
        });
        *inquiry_counts.entry(inquiry_classification).or_insert(0) += 1;
    }

    for (classification, count) in inquiry_counts {
        println!("INQUIRY_CLASSIFICATION classification={classification:?} count={count}");
    }

    let prepared_issue_ids = prepared_issues
        .iter()
        .map(|prepared| prepared.issue_id.into_inner())
        .collect::<BTreeSet<_>>();
    let target_tracker_ids = TARGET_TRACKERS
        .iter()
        .map(|(tracker_id, _)| *tracker_id)
        .collect::<BTreeSet<_>>();
    let mut relation_values = Vec::new();
    let mut relation_endpoints = BTreeSet::new();
    let mut skipped_relations = 0;
    for relation in issue_relations {
        let first_tracker_id = tracker_by_issue.get(&relation.issue_id).copied();
        let second_tracker_id = tracker_by_issue.get(&relation.issue_to_id).copied();
        if !first_tracker_id.is_some_and(|tracker_id| target_tracker_ids.contains(&tracker_id))
            || !second_tracker_id.is_some_and(|tracker_id| target_tracker_ids.contains(&tracker_id))
        {
            skipped_relations += 1;
            report.warning(format!(
                "Redmine relation {} ({}) は対象 tracker 同士ではないため移行しません: {} -> {}",
                relation.id, relation.relation_type, relation.issue_id, relation.issue_to_id
            ));
            continue;
        }
        if !prepared_issue_ids.contains(&relation.issue_id)
            || !prepared_issue_ids.contains(&relation.issue_to_id)
        {
            skipped_relations += 1;
            report.error(format!(
                "Redmine relation {} の両端 issue が移行対象として準備されていません: {} -> {}",
                relation.id, relation.issue_id, relation.issue_to_id
            ));
            continue;
        }

        let endpoint_key = if relation.issue_id < relation.issue_to_id {
            (relation.issue_id, relation.issue_to_id)
        } else {
            (relation.issue_to_id, relation.issue_id)
        };
        if !relation_endpoints.insert(endpoint_key) {
            skipped_relations += 1;
            report.warning(format!(
                "Redmine relation {} は同じ issue pair の重複 relation のため統合します: {} -> {}",
                relation.id, relation.issue_id, relation.issue_to_id
            ));
            continue;
        }
        let first_issue_id = RedmineIssueId::try_new(endpoint_key.0)?;
        let second_issue_id = RedmineIssueId::try_new(endpoint_key.1)?;
        relation_values.push(RedmineIssueRelation::new(first_issue_id, second_issue_id)?);
    }
    let relation_batch = RedmineIssueRelationBatch::new(relation_values)?;
    println!(
        "REDMINE_RELATIONS candidates={} skipped_or_reported={}",
        relation_batch.relations().len(),
        skipped_relations
    );

    report.print();
    if !report.errors.is_empty() {
        bail!("移行対象の事前検証に失敗しました。DB は変更していません");
    }

    match mode {
        Mode::Plan => {
            for prepared in &prepared_issues {
                let result = usecase.verify_issue(&prepared.issue).await?;
                println!(
                    "PLAN project={} issue={} tracker={:?} classification={:?} result={result:?}",
                    prepared.project_id,
                    prepared.issue_id.into_inner(),
                    prepared.tracker_name,
                    prepared.inquiry_classification
                );
            }
        }
        Mode::Verify => {
            for prepared in &prepared_issues {
                let result = usecase.verify_issue(&prepared.issue).await?;
                println!(
                    "VERIFY project={} issue={} tracker={:?} classification={:?} result={result:?}",
                    prepared.project_id,
                    prepared.issue_id.into_inner(),
                    prepared.tracker_name,
                    prepared.inquiry_classification
                );
            }
        }
        Mode::Import => {
            for prepared in prepared_issues {
                let issue_id = prepared.issue_id.into_inner();
                let result = usecase.import_issue(prepared.issue).await?;
                println!(
                    "IMPORT project={} issue={issue_id} tracker={:?} classification={:?} result={result:?}",
                    prepared.project_id, prepared.tracker_name, prepared.inquiry_classification
                );
            }
            let result = usecase.import_answer_relations(relation_batch).await?;
            println!(
                "IMPORT answer_relations inserted={} already_exists={}",
                result.inserted(),
                result.already_exists()
            );
        }
    }

    Ok(())
}

fn parse_args() -> Result<(Mode, PathBuf)> {
    let mut args = env::args().skip(1);
    let mode = args
        .next()
        .context("モードを指定してください: plan / import / verify")
        .and_then(|mode| Mode::parse(&mode))?;
    let mut config_path = env::var_os("REDMINE_IMPORT_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("server/redmine-importer/config/redmine-import.json"));

    while let Some(argument) = args.next() {
        if argument == "--config" {
            config_path = PathBuf::from(
                args.next()
                    .context("--config には設定ファイルのパスが必要です")?,
            );
        } else {
            bail!("不明な引数です: {argument}");
        }
    }

    Ok((mode, config_path))
}
