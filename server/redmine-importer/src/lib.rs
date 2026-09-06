use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Timelike, Utc};
use domain::form::{
    answer::{AnswerId, AnswerPublication, AnswerStatus, RedmineIssueId, RedmineUserSnapshot},
    models::FormId,
};
use reqwest::{StatusCode, header};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use usecase::redmine_import::{RedmineIssueInput, RedmineJournalInput};

use domain::form::comment_attachment::MAX_COMMENT_ATTACHMENT_SIZE;

pub const PUBLIC_TRACKER_ID: i64 = 19;

pub const TARGET_TRACKERS: &[(i64, &str)] = &[
    (1, "不具合"),
    (2, "アイデア"),
    (4, "お問い合わせ"),
    (5, "ご意見ご感想"),
    (6, "通報"),
    (9, "不具合報告"),
    (12, "公共建築"),
    (15, "修繕依頼"),
    (18, "イベント"),
    (19, "アイデア提案"),
    (22, "デザイン"),
    (29, "運営会議"),
    (33, "ゲーム内処罰エビデンス"),
    (34, "アイデア会議"),
    (35, "Discord処罰エビデンス"),
    (36, "不要保護報告"),
];

pub const EXCLUDED_TRACKERS: &[(i64, &str)] = &[
    (7, "TT申請"),
    (8, "道路整備"),
    (10, "(過去ログ)ゲーム内処罰エビデンス"),
    (20, "動画制作"),
    (21, "公式HP編集"),
    (24, "Webアプリ開発"),
    (28, "WikiEditor申請"),
    (30, "Observer申請"),
    (32, "公式生配信"),
];

#[derive(Debug, Deserialize)]
pub struct Config {
    pub redmine_projects: Vec<RedmineProject>,
    pub page_size: u64,
    pub detail_concurrency: usize,
    pub max_retries: u32,
    pub retry_base_delay_ms: u64,
    pub status_mapping: BTreeMap<String, String>,
    #[serde(default)]
    pub status_label_mappings: BTreeMap<i64, BTreeMap<String, String>>,
    pub tracker_mappings: BTreeMap<i64, TrackerMapping>,
    pub inquiry_mappings: BTreeMap<InquiryRoute, FormMapping>,
    pub excluded_trackers: BTreeMap<i64, String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RedmineProject {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FormMapping {
    #[serde(default)]
    pub redmine_name: Option<String>,
    pub form_id: String,
    pub form_title: String,
    pub question_mappings: BTreeMap<String, QuestionValueSource>,
    pub custom_field_content_template_key: String,
    #[serde(default)]
    pub custom_field_question_mappings: BTreeMap<i64, String>,
    #[serde(default)]
    pub labels: Vec<String>,
}

pub type TrackerMapping = FormMapping;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum InquiryRoute {
    Mod,
    Login,
    ExperienceOverflow,
    Appeal,
}

impl InquiryRoute {
    pub const SPECIAL: [Self; 4] = [
        Self::Mod,
        Self::Login,
        Self::ExperienceOverflow,
        Self::Appeal,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Mod => "mod",
            Self::Login => "login",
            Self::ExperienceOverflow => "experience_overflow",
            Self::Appeal => "appeal",
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum QuestionValueSource {
    Subject,
    Description,
    SubjectAndDescription,
    ModName,
    FirstUrl,
    Ipv4Addresses,
    #[serde(rename = "static")]
    Static {
        value: String,
    },
}

impl QuestionValueSource {
    fn resolve(&self, subject: &str, description: &str) -> String {
        match self {
            Self::Subject => subject.to_owned(),
            Self::Description => description.to_owned(),
            Self::SubjectAndDescription => {
                if description.is_empty() {
                    subject.to_owned()
                } else {
                    format!("{subject}\n\n{description}")
                }
            }
            Self::ModName => extract_mod_name(subject, description),
            Self::FirstUrl => first_url(subject, description),
            Self::Ipv4Addresses => extract_ipv4_addresses(subject, description),
            Self::Static { value } => value.clone(),
        }
    }
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = fs::read(path)
            .with_context(|| format!("移行設定を読み込めません: {}", path.display()))?;
        let config: Self = serde_json::from_slice(&content)
            .with_context(|| format!("移行設定の JSON が不正です: {}", path.display()))?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        if self.redmine_projects.is_empty() {
            bail!("redmine_projects は 1 件以上指定してください");
        }
        let project_ids = self
            .redmine_projects
            .iter()
            .map(|project| project.id)
            .collect::<BTreeSet<_>>();
        if project_ids.len() != self.redmine_projects.len() {
            bail!("redmine_projects に重複した project ID があります");
        }
        for project in &self.redmine_projects {
            if project.id <= 0 {
                bail!(
                    "Redmine project ID は正の整数でなければなりません: {}",
                    project.id
                );
            }
            if project.name.trim().is_empty() {
                bail!("Redmine project {} の name は必須です", project.id);
            }
        }

        if self.page_size == 0 || self.page_size > 100 {
            bail!("page_size は 1 以上 100 以下でなければなりません");
        }
        if self.detail_concurrency == 0 || self.detail_concurrency > 32 {
            bail!("detail_concurrency は 1 以上 32 以下でなければなりません");
        }

        let target_tracker_ids = TARGET_TRACKERS
            .iter()
            .map(|(id, _)| *id)
            .collect::<BTreeSet<_>>();
        let configured_trackers = self
            .tracker_mappings
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        if configured_trackers != target_tracker_ids {
            bail!("tracker_mappings は対象 tracker の一覧と完全一致させてください");
        }

        let excluded_trackers = EXCLUDED_TRACKERS
            .iter()
            .copied()
            .collect::<BTreeMap<_, _>>();
        let excluded_trackers = excluded_trackers
            .into_iter()
            .map(|(id, name)| (id, name.to_string()))
            .collect::<BTreeMap<_, _>>();
        if self.excluded_trackers != excluded_trackers {
            bail!("excluded_trackers は対象外 tracker の一覧と完全一致させてください");
        }

        let mut status_label_names = BTreeSet::new();
        for (tracker_id, status_labels) in &self.status_label_mappings {
            if !target_tracker_ids.contains(tracker_id) {
                bail!("status_label_mappings に対象外 tracker が指定されています: {tracker_id}");
            }
            for (status_name, label_name) in status_labels {
                if !self.status_mapping.contains_key(status_name) {
                    bail!(
                        "tracker {tracker_id} の status label mapping に未定義の Redmine status があります: {status_name:?}"
                    );
                }
                if label_name.trim().is_empty() {
                    bail!(
                        "tracker {tracker_id} の status label mapping に空の label name があります"
                    );
                }
                if !status_label_names.insert(label_name) {
                    bail!("status_label_mappings に重複した label name があります: {label_name:?}");
                }
            }
        }

        for (tracker_id, mapping) in &self.tracker_mappings {
            let expected_name = TARGET_TRACKERS
                .iter()
                .find(|(id, _)| id == tracker_id)
                .map(|(_, name)| *name)
                .expect("tracker_mappings keys were validated above");
            if mapping.redmine_name.as_deref() != Some(expected_name) {
                bail!(
                    "tracker {tracker_id} の redmine_name が不正です: expected={expected_name:?}, actual={:?}",
                    mapping.redmine_name
                );
            }
            validate_form_mapping(&format!("tracker {tracker_id}"), mapping)?;
        }

        let configured_inquiry_routes = self
            .inquiry_mappings
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        let expected_inquiry_routes = InquiryRoute::SPECIAL.into_iter().collect::<BTreeSet<_>>();
        if configured_inquiry_routes != expected_inquiry_routes {
            bail!("inquiry_mappings は専用問い合わせ route の一覧と完全一致させてください");
        }
        for route in InquiryRoute::SPECIAL {
            let mapping = self
                .inquiry_mappings
                .get(&route)
                .expect("inquiry_mappings keys were validated above");
            validate_form_mapping(&format!("inquiry route {:?}", route.name()), mapping)?;
        }

        for (status_name, mapped_status) in &self.status_mapping {
            AnswerStatus::try_from(mapped_status.clone()).with_context(|| {
                format!("status_mapping の Portal status が不正です: {status_name:?}")
            })?;
        }

        Ok(())
    }

    pub fn status_for(&self, redmine_status: &str) -> Result<AnswerStatus> {
        let mapped_status = self
            .status_mapping
            .get(redmine_status)
            .with_context(|| format!("未対応の Redmine status です: {redmine_status:?}"))?;
        AnswerStatus::try_from(mapped_status.clone())
            .with_context(|| format!("Portal status への変換に失敗しました: {redmine_status:?}"))
    }

    pub fn status_label_for(&self, tracker_id: i64, redmine_status: &str) -> Option<&str> {
        self.status_label_mappings
            .get(&tracker_id)
            .and_then(|status_labels| status_labels.get(redmine_status))
            .map(String::as_str)
    }

    pub fn validate_statuses(&self, statuses: &[NamedId]) -> Result<()> {
        let status_ids = statuses
            .iter()
            .map(|status| status.id)
            .collect::<BTreeSet<_>>();
        if status_ids.len() != statuses.len() || statuses.iter().any(|status| status.id <= 0) {
            bail!("Redmine status の ID が重複または不正です");
        }
        let actual_status_names = statuses
            .iter()
            .map(|status| status.name.as_str())
            .collect::<BTreeSet<_>>();
        if actual_status_names.len() != statuses.len() {
            bail!("Redmine status の名前が重複しています");
        }
        let configured_status_names = self
            .status_mapping
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if actual_status_names != configured_status_names {
            bail!(
                "Redmine status mapping は API が返した status の一覧と完全一致させてください: actual={actual_status_names:?}, configured={configured_status_names:?}"
            );
        }
        Ok(())
    }

    pub fn mapping_for(&self, tracker_id: i64) -> Result<&TrackerMapping> {
        self.tracker_mappings
            .get(&tracker_id)
            .with_context(|| format!("対象 tracker の mapping がありません: {tracker_id}"))
    }

    pub fn form_mapping_for(
        &self,
        tracker_id: i64,
        route: Option<InquiryRoute>,
    ) -> Result<&FormMapping> {
        match route {
            Some(route) if tracker_id == 4 => {
                self.inquiry_mappings.get(&route).with_context(|| {
                    format!(
                        "問い合わせ route の mapping がありません: {:?}",
                        route.name()
                    )
                })
            }
            Some(_) => bail!("専用問い合わせ route は tracker 4 にだけ指定できます"),
            None => Ok(self.mapping_for(tracker_id)?),
        }
    }

    pub fn project_name_for(&self, project_id: i64) -> Result<&str> {
        self.redmine_projects
            .iter()
            .find(|project| project.id == project_id)
            .map(|project| project.name.as_str())
            .with_context(|| format!("設定されていない Redmine project ID です: {project_id}"))
    }

    pub fn form_id_for(&self, tracker_id: i64) -> Result<FormId> {
        let mapping = self.form_mapping_for(tracker_id, None)?;
        uuid::Uuid::parse_str(&mapping.form_id)
            .map(Into::into)
            .with_context(|| {
                format!(
                    "tracker {tracker_id} の form_id が UUID ではありません: {:?}",
                    mapping.form_id
                )
            })
    }

    pub fn form_id_for_inquiry(&self, route: InquiryRoute) -> Result<FormId> {
        let mapping = self.form_mapping_for(4, Some(route))?;
        uuid::Uuid::parse_str(&mapping.form_id)
            .map(Into::into)
            .with_context(|| {
                format!(
                    "inquiry route {:?} の form_id が UUID ではありません: {:?}",
                    route.name(),
                    mapping.form_id
                )
            })
    }

    pub fn question_values_for(
        &self,
        tracker_id: i64,
        issue: &RedmineIssue,
    ) -> Result<BTreeMap<String, String>> {
        self.question_values_for_mapping(self.form_mapping_for(tracker_id, None)?, issue)
    }

    pub fn question_values_for_inquiry(
        &self,
        route: InquiryRoute,
        issue: &RedmineIssue,
    ) -> Result<BTreeMap<String, String>> {
        self.question_values_for_mapping(self.form_mapping_for(4, Some(route))?, issue)
    }

    fn question_values_for_mapping(
        &self,
        mapping: &FormMapping,
        issue: &RedmineIssue,
    ) -> Result<BTreeMap<String, String>> {
        let description = issue.description.as_deref().unwrap_or_default();
        let mut values = mapping
            .question_mappings
            .iter()
            .map(|(template_key, source)| {
                (
                    template_key.clone(),
                    source.resolve(&issue.subject, description),
                )
            })
            .collect::<BTreeMap<_, _>>();

        for (field_id, template_key) in &mapping.custom_field_question_mappings {
            if let Some(value) = issue
                .custom_fields
                .iter()
                .find(|field| field.id == *field_id)
                .and_then(|field| custom_field_value(&field.value))
            {
                values.insert(template_key.clone(), value);
            }
        }

        let mapped_field_ids = mapping
            .custom_field_question_mappings
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        if let Some(custom_field_block) =
            custom_field_block(&issue.custom_fields, &mapped_field_ids)
        {
            let content = values
                .get_mut(&mapping.custom_field_content_template_key)
                .with_context(|| {
                    format!(
                        "custom field 本文 mapping が見つかりません: {:?}",
                        mapping.custom_field_content_template_key
                    )
                })?;
            if !content.is_empty() {
                content.push_str("\n\n");
            }
            content.push_str(&custom_field_block);
        }

        Ok(values)
    }
}

fn validate_form_mapping(scope: &str, mapping: &FormMapping) -> Result<()> {
    if mapping.form_id.trim().is_empty() || mapping.form_title.trim().is_empty() {
        bail!("{scope} の form_id/form_title は必須です");
    }
    uuid::Uuid::parse_str(&mapping.form_id).with_context(|| {
        format!(
            "{scope} の form_id が UUID ではありません: {:?}",
            mapping.form_id
        )
    })?;
    if mapping.question_mappings.is_empty() {
        bail!("{scope} の question_mappings は空にできません");
    }
    if mapping.custom_field_content_template_key.trim().is_empty() {
        bail!("{scope} の custom_field_content_template_key は必須です");
    }
    if !mapping
        .question_mappings
        .contains_key(&mapping.custom_field_content_template_key)
    {
        bail!(
            "{scope} の custom_field_content_template_key は question_mappings に存在しなければなりません: {:?}",
            mapping.custom_field_content_template_key
        );
    }
    let mut custom_field_question_keys = BTreeSet::new();
    for (field_id, template_key) in &mapping.custom_field_question_mappings {
        if *field_id <= 0 {
            bail!("{scope} の custom field ID は正の整数でなければなりません: {field_id}");
        }
        if template_key.trim().is_empty() {
            bail!("{scope} の custom field mapping に空の template key があります");
        }
        if mapping.question_mappings.contains_key(template_key) {
            bail!(
                "{scope} の custom field mapping は既存の question mapping と重複しています: {template_key:?}"
            );
        }
        if !custom_field_question_keys.insert(template_key) {
            bail!(
                "{scope} の custom field mapping に重複した template key があります: {template_key:?}"
            );
        }
    }
    if mapping.labels.iter().any(|label| label.trim().is_empty()) {
        bail!("{scope} の label mapping に空の値があります");
    }
    if mapping.labels.iter().collect::<BTreeSet<_>>().len() != mapping.labels.len() {
        bail!("{scope} の label mapping に重複があります");
    }
    for (template_key, source) in &mapping.question_mappings {
        if template_key.trim().is_empty() {
            bail!("{scope} の question mapping に空の template key があります");
        }
        if let QuestionValueSource::Static { value } = source
            && value.trim().is_empty()
        {
            bail!("{scope} の static question value に空の値があります");
        }
    }
    Ok(())
}

fn custom_field_value(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(value) if value.trim().is_empty() => None,
        Value::String(value) => Some(value.clone()),
        Value::Array(values) if values.is_empty() => None,
        Value::Object(values) if values.is_empty() => None,
        value => Some(value.to_string()),
    }
}

const CONTACT_CUSTOM_FIELD_ID: i64 = 1;
const ID_CUSTOM_FIELD_ID: i64 = 2;

fn redmine_author_display_name(
    original_name: &str,
    custom_fields: &[RedmineCustomField],
) -> String {
    let contact = meaningful_author_custom_field_value(custom_fields, CONTACT_CUSTOM_FIELD_ID);
    let id = meaningful_author_custom_field_value(custom_fields, ID_CUSTOM_FIELD_ID);

    match (id, contact) {
        (Some(id), Some(contact)) => format!("ID: {id} / 連絡先: {contact}"),
        (Some(id), None) | (None, Some(id)) => id,
        (None, None) => original_name.to_owned(),
    }
}

fn meaningful_author_custom_field_value(
    custom_fields: &[RedmineCustomField],
    field_id: i64,
) -> Option<String> {
    custom_fields
        .iter()
        .filter(|field| field.id == field_id)
        .find_map(|field| {
            custom_field_value(&field.value).and_then(|value| {
                let value = value.trim();
                if is_unfilled_author_value(value) {
                    None
                } else {
                    Some(value.to_owned())
                }
            })
        })
}

fn is_unfilled_author_value(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    value.is_empty()
        || !value.chars().any(|character| character.is_alphanumeric())
        || matches!(
            normalized.as_str(),
            "なし"
                | "無し"
                | "不明"
                | "未回答"
                | "未記入"
                | "未入力"
                | "無回答"
                | "空欄"
                | "回答なし"
                | "回答無し"
                | "記載なし"
                | "記載無し"
                | "該当なし"
                | "該当無し"
                | "特になし"
                | "特に無し"
                | "n/a"
                | "na"
                | "none"
                | "unknown"
        )
}

fn custom_field_block(
    fields: &[RedmineCustomField],
    excluded_field_ids: &BTreeSet<i64>,
) -> Option<String> {
    let fields = fields
        .iter()
        .filter(|field| !excluded_field_ids.contains(&field.id))
        .filter_map(|field| {
            custom_field_value(&field.value)
                .map(|value| format!("- {} (ID: {}): {value}", field.name, field.id))
        })
        .collect::<Vec<_>>();
    if fields.is_empty() {
        return None;
    }

    Some(format!(
        "---\nRedmine カスタムフィールド:\n{}",
        fields.join("\n")
    ))
}

/// tracker 4 の問い合わせを専用フォームへ振り分ける分類です。
///
/// 件名にカテゴリが明記されている場合を優先し、件名にカテゴリがない場合は、
/// 経験値とオーバーフローが近接して現れるときだけ高信頼な XP 問い合わせとして扱います。
/// 一般的なキーワードだけでは専用フォームへ振り分けません。
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum InquiryClassification {
    Generic,
    Mod,
    Login,
    ExperienceOverflow,
    Appeal,
}

impl InquiryClassification {
    pub fn special_route(self) -> Option<InquiryRoute> {
        match self {
            Self::Generic => None,
            Self::Mod => Some(InquiryRoute::Mod),
            Self::Login => Some(InquiryRoute::Login),
            Self::ExperienceOverflow => Some(InquiryRoute::ExperienceOverflow),
            Self::Appeal => Some(InquiryRoute::Appeal),
        }
    }
}

pub fn classify_inquiry(subject: &str, description: Option<&str>) -> InquiryClassification {
    if let Some(classification) = explicit_subject_classification(subject) {
        return classification;
    }

    let description = description.unwrap_or_default();
    if has_nearby_terms(subject, "経験値", "オーバーフロー", 20)
        || has_nearby_terms(description, "経験値", "オーバーフロー", 20)
    {
        InquiryClassification::ExperienceOverflow
    } else {
        InquiryClassification::Generic
    }
}

fn explicit_subject_classification(subject: &str) -> Option<InquiryClassification> {
    let lowercase = subject.to_ascii_lowercase();
    if (subject.contains("Mod") || subject.contains("MOD") || lowercase.contains("mod"))
        && subject.contains("使用可否")
    {
        return Some(InquiryClassification::Mod);
    }
    if subject.contains("ログインできない")
        || subject.contains("ログインできなく")
        || subject.contains("ログインできません")
        || lowercase.contains("blacklisted from server")
        || lowercase.contains("blacklisted from the server")
    {
        return Some(InquiryClassification::Login);
    }
    if subject.contains("処罰への異議申し立て") || subject.contains("BAN解除申立") {
        return Some(InquiryClassification::Appeal);
    }
    if has_nearby_terms(subject, "経験値", "オーバーフロー", 20) {
        return Some(InquiryClassification::ExperienceOverflow);
    }
    None
}

fn has_nearby_terms(text: &str, first: &str, second: &str, max_gap: usize) -> bool {
    let compact = text
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    nearby_in_order(&compact, first, second, max_gap)
        || nearby_in_order(&compact, second, first, max_gap)
}

fn nearby_in_order(text: &str, first: &str, second: &str, max_gap: usize) -> bool {
    text.match_indices(first).any(|(start, _)| {
        let rest = &text[start + first.len()..];
        rest.find(second)
            .is_some_and(|offset| rest[..offset].chars().count() <= max_gap)
    })
}

fn extract_mod_name(subject: &str, _description: &str) -> String {
    let marker = subject
        .find("使用可否")
        .map(|index| &subject[index + "使用可否".len()..])
        .map(str::trim)
        .map(|value| value.trim_start_matches(['：', ':', '-', '—', ' ']).trim())
        .filter(|value| {
            !value.is_empty()
                && *value != "のお問い合わせ"
                && *value != "について"
                && *value != "の問い合わせ"
        });
    if let Some(value) = marker {
        return value.to_owned();
    }

    "（件名から特定できず）".to_string()
}

fn first_url(subject: &str, description: &str) -> String {
    subject
        .split_whitespace()
        .chain(description.split_whitespace())
        .find_map(|token| {
            let token = token.trim_matches(|character: char| {
                matches!(
                    character,
                    '<' | '>'
                        | '('
                        | ')'
                        | '['
                        | ']'
                        | '{'
                        | '}'
                        | '"'
                        | '\''
                        | '、'
                        | '。'
                        | '，'
                        | ','
                )
            });
            (token.starts_with("https://") || token.starts_with("http://"))
                .then(|| token.to_owned())
        })
        .unwrap_or_else(|| "（URLの記載なし）".to_string())
}

fn extract_ipv4_addresses(subject: &str, description: &str) -> String {
    let mut addresses = Vec::new();
    for token in subject
        .split(|character: char| !(character.is_ascii_digit() || character == '.'))
        .chain(
            description.split(|character: char| !(character.is_ascii_digit() || character == '.')),
        )
    {
        if token.is_empty() || token.matches('.').count() != 3 {
            continue;
        }
        let parts = token.split('.').collect::<Vec<_>>();
        if parts.len() == 4
            && parts
                .iter()
                .all(|part| !part.is_empty() && part.parse::<u8>().is_ok())
            && !addresses.iter().any(|address| address == token)
        {
            addresses.push(token.to_string());
        }
    }
    addresses.join("\n")
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct RedmineIssueRelation {
    pub id: i64,
    pub issue_id: i64,
    pub issue_to_id: i64,
    pub relation_type: String,
}

pub fn unique_issue_relations(stubs: &[IssueStub]) -> Result<Vec<RedmineIssueRelation>> {
    let mut relations = BTreeMap::new();
    for stub in stubs {
        for relation in &stub.relations {
            if relation.id <= 0 || relation.issue_id <= 0 || relation.issue_to_id <= 0 {
                bail!("Redmine relation の ID または endpoint が不正です: {relation:?}");
            }
            if relation.issue_id == relation.issue_to_id {
                bail!("Redmine relation が自己参照しています: {relation:?}");
            }
            let (issue_id, issue_to_id) = if relation.issue_id < relation.issue_to_id {
                (relation.issue_id, relation.issue_to_id)
            } else {
                (relation.issue_to_id, relation.issue_id)
            };
            let normalized = RedmineIssueRelation {
                id: relation.id,
                issue_id,
                issue_to_id,
                relation_type: relation.relation_type.clone(),
            };
            if let Some(existing) = relations.insert(relation.id, normalized.clone())
                && existing != normalized
            {
                bail!(
                    "同じ Redmine relation ID に異なる内容があります: id={}, existing={existing:?}, received={relation:?}",
                    relation.id
                );
            }
        }
    }
    Ok(relations.into_values().collect())
}

#[derive(Clone)]
enum ApiAuthorization {
    RedmineApiKey(String),
    PortalSession(String),
}

#[derive(Clone)]
struct ApiClient {
    client: reqwest::Client,
    base_url: String,
    authorization: ApiAuthorization,
    max_retries: u32,
    retry_base_delay_ms: u64,
}

struct MultipartPart {
    file_name: String,
    content_type: String,
    content: Vec<u8>,
}

impl ApiClient {
    fn new(
        base_url: String,
        authorization: ApiAuthorization,
        max_retries: u32,
        retry_base_delay_ms: u64,
    ) -> Result<Self> {
        let base_url = base_url.trim_end_matches('/').to_owned();
        if base_url.trim().is_empty() {
            bail!("API base URL が空です");
        }

        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent("seichi-portal-redmine-importer")
                .timeout(Duration::from_secs(30))
                .build()?,
            base_url,
            authorization,
            max_retries,
            retry_base_delay_ms,
        })
    }

    fn url(&self, path: &str) -> String {
        if path.starts_with("http://") || path.starts_with("https://") {
            path.to_owned()
        } else {
            format!("{}{path}", self.base_url)
        }
    }

    fn authorize(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.authorization {
            ApiAuthorization::RedmineApiKey(api_key) => {
                request.header("X-Redmine-API-Key", api_key)
            }
            ApiAuthorization::PortalSession(session_id) => request.bearer_auth(session_id),
        }
    }

    async fn get_response(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<reqwest::Response> {
        let url = self.url(path);
        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            let response = self
                .authorize(self.client.get(&url).query(query))
                .send()
                .await;

            let response = match response {
                Ok(response) => response,
                Err(error) => {
                    last_error = Some(format!("GET {path} に失敗しました: {error}"));
                    if attempt == self.max_retries {
                        break;
                    }
                    tokio::time::sleep(retry_delay(attempt, self.retry_base_delay_ms)).await;
                    continue;
                }
            };

            let status = response.status();
            if status.is_success() {
                return Ok(response);
            }

            let body = response.text().await.unwrap_or_default();
            let message = format!(
                "GET {path} が HTTP {} で失敗しました: {}",
                status,
                truncate_for_error(&body),
            );
            if !is_retryable_status(status) || attempt == self.max_retries {
                bail!(message);
            }
            last_error = Some(message);
            tokio::time::sleep(retry_delay(attempt, self.retry_base_delay_ms)).await;
        }

        bail!(last_error.unwrap_or_else(|| format!("GET {path} に失敗しました")))
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T> {
        self.get_response(path, query)
            .await?
            .json::<T>()
            .await
            .with_context(|| format!("GET {path} の JSON が不正です"))
    }

    async fn get_bytes(&self, path: &str) -> Result<(String, Vec<u8>)> {
        let mut response = self.get_response(path, &[]).await?;
        if response
            .content_length()
            .is_some_and(|size| size > MAX_COMMENT_ATTACHMENT_SIZE)
        {
            bail!(
                "Redmine attachment {} はサイズ上限 {} bytes を超えています",
                path,
                MAX_COMMENT_ATTACHMENT_SIZE
            );
        }
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .map(|value| {
                value
                    .to_str()
                    .context("Redmine attachment の Content-Type が不正です")
                    .map(str::to_owned)
            })
            .transpose()?
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "application/octet-stream".to_owned());
        let capacity = response
            .content_length()
            .unwrap_or_default()
            .min(MAX_COMMENT_ATTACHMENT_SIZE) as usize;
        let mut content = Vec::with_capacity(capacity);
        while let Some(chunk) = response.chunk().await? {
            let next_size = content
                .len()
                .checked_add(chunk.len())
                .context("Redmine attachment のサイズ計算がオーバーフローしました")?;
            if next_size as u64 > MAX_COMMENT_ATTACHMENT_SIZE {
                bail!(
                    "Redmine attachment {} はサイズ上限 {} bytes を超えています",
                    path,
                    MAX_COMMENT_ATTACHMENT_SIZE
                );
            }
            content.extend_from_slice(&chunk);
        }
        Ok((content_type, content))
    }

    async fn post_multipart(&self, path: &str, parts: &[MultipartPart]) -> Result<()> {
        let mut last_error = None;
        for attempt in 0..=self.max_retries {
            let form = parts
                .iter()
                .try_fold(reqwest::multipart::Form::new(), |form, part| {
                    let part = reqwest::multipart::Part::bytes(part.content.clone())
                        .file_name(part.file_name.clone())
                        .mime_str(&part.content_type)
                        .context("Portal attachment の Content-Type が不正です")?;
                    Ok::<_, anyhow::Error>(form.part("file", part))
                })?;
            let response = self
                .authorize(self.client.post(self.url(path)).multipart(form))
                .send()
                .await;
            let response = match response {
                Ok(response) => response,
                Err(error) => {
                    last_error = Some(format!("POST {path} に失敗しました: {error}"));
                    if attempt == self.max_retries {
                        break;
                    }
                    tokio::time::sleep(retry_delay(attempt, self.retry_base_delay_ms)).await;
                    continue;
                }
            };
            if response.status().is_success() {
                return Ok(());
            }
            let status = response.status();
            let retry_after = response
                .headers()
                .get(header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .map(Duration::from_secs);
            let body = response.text().await.unwrap_or_default();
            let message = format!(
                "POST {path} が HTTP {} で失敗しました: {}",
                status,
                truncate_for_error(&body)
            );
            if !is_retryable_status(status) || attempt == self.max_retries {
                bail!(message);
            }
            last_error = Some(message);
            tokio::time::sleep(
                retry_after.unwrap_or_else(|| retry_delay(attempt, self.retry_base_delay_ms)),
            )
            .await;
        }
        bail!(last_error.unwrap_or_else(|| format!("POST {path} に失敗しました")))
    }
}

#[derive(Clone)]
pub struct RedmineApi {
    http: ApiClient,
    project_ids: Vec<i64>,
}

impl RedmineApi {
    pub fn from_env(config: &Config) -> Result<Self> {
        let base_url =
            std::env::var("REDMINE_BASE_URL").context("REDMINE_BASE_URL が設定されていません")?;
        let api_key =
            std::env::var("REDMINE_API_KEY").context("REDMINE_API_KEY が設定されていません")?;
        Self::new(
            base_url,
            api_key,
            config
                .redmine_projects
                .iter()
                .map(|project| project.id)
                .collect(),
            config.max_retries,
            config.retry_base_delay_ms,
        )
    }

    pub fn new(
        base_url: String,
        api_key: String,
        project_ids: Vec<i64>,
        max_retries: u32,
        retry_base_delay_ms: u64,
    ) -> Result<Self> {
        if base_url.trim().is_empty() {
            bail!("Redmine base URL が空です");
        }
        if api_key.trim().is_empty() {
            bail!("Redmine API key が空です");
        }
        if project_ids.is_empty() {
            bail!("Redmine project ID を 1 件以上指定してください");
        }
        if project_ids.iter().any(|project_id| *project_id <= 0) {
            bail!("Redmine project ID は正の整数でなければなりません");
        }
        if project_ids.iter().collect::<BTreeSet<_>>().len() != project_ids.len() {
            bail!("Redmine project ID に重複があります");
        }

        Ok(Self {
            http: ApiClient::new(
                base_url,
                ApiAuthorization::RedmineApiKey(api_key),
                max_retries,
                retry_base_delay_ms,
            )?,
            project_ids,
        })
    }

    pub async fn fetch_issue_statuses(&self) -> Result<Vec<NamedId>> {
        let response: IssueStatusesResponse =
            self.http.get_json("/issue_statuses.json", &[]).await?;
        Ok(response.issue_statuses)
    }

    pub async fn fetch_issue_stubs(&self, page_size: u64) -> Result<Vec<IssueStub>> {
        let mut stubs = Vec::new();
        let mut issue_stubs = BTreeMap::new();
        let configured_project_ids = self.project_ids.iter().copied().collect::<BTreeSet<_>>();

        for project_id in &self.project_ids {
            let mut offset = 0;

            loop {
                let page: IssuePage = self
                    .http
                    .get_json(
                        "/issues.json",
                        &[
                            ("project_id", project_id.to_string()),
                            // 親 project の一覧に子 project の issue が混ざるため、設定した
                            // project 自身の issue だけを取得する。子 project は個別に列挙する。
                            ("subproject_id", "!*".to_string()),
                            ("status_id", "*".to_string()),
                            ("include", "relations".to_string()),
                            ("limit", page_size.to_string()),
                            ("offset", offset.to_string()),
                            ("sort", "id:asc".to_string()),
                        ],
                    )
                    .await?;
                if page.offset != offset {
                    bail!(
                        "Redmine project {project_id} の pagination の offset が一致しません: requested={offset}, received={}",
                        page.offset
                    );
                }
                let page_len = page.issues.len() as u64;
                for issue in page.issues {
                    if !configured_project_ids.contains(&issue.project.id) {
                        bail!(
                            "Redmine project {project_id} の一覧に設定外 project の issue が含まれています: issue={}, project={} ({:?})",
                            issue.id,
                            issue.project.id,
                            issue.project.name
                        );
                    }

                    let stub = IssueStub {
                        project_id: issue.project.id,
                        id: issue.id,
                        tracker_id: issue.tracker.id,
                        tracker_name: issue.tracker.name,
                        relations: issue.relations,
                    };
                    if let Some(existing) = issue_stubs.insert(issue.id, stub.clone())
                        && existing != stub
                    {
                        bail!(
                            "Redmine の複数 project 一覧で issue {} の内容が一致しません: existing={existing:?}, received={stub:?}",
                            issue.id
                        );
                    }
                }

                let Some(next_offset) = next_page_offset(offset, page.total_count, page_len)?
                else {
                    break;
                };
                offset = next_offset;
            }
        }

        stubs.extend(issue_stubs.into_values());
        stubs.sort_by_key(|stub| stub.id);
        Ok(stubs)
    }

    pub async fn fetch_issue(&self, issue_id: RedmineIssueId) -> Result<RedmineIssue> {
        let issue_id = issue_id.into_inner();
        let response: IssueResponse = self
            .http
            .get_json(
                &format!("/issues/{issue_id}.json"),
                &[("include", "journals,attachments".to_string())],
            )
            .await?;
        if response.issue.id != issue_id {
            bail!(
                "Redmine issue detail の ID が一致しません: requested={issue_id}, received={}",
                response.issue.id
            );
        }
        Ok(response.issue)
    }

    pub async fn download_attachment(
        &self,
        attachment: &RedmineAttachment,
    ) -> Result<DownloadedRedmineAttachment> {
        if attachment.id <= 0 {
            bail!("Redmine attachment ID が不正です: {}", attachment.id);
        }
        if attachment
            .filesize
            .is_some_and(|size| size > MAX_COMMENT_ATTACHMENT_SIZE)
        {
            bail!(
                "Redmine attachment {} はサイズ上限 {} bytes を超えています",
                attachment.id,
                MAX_COMMENT_ATTACHMENT_SIZE
            );
        }
        let path = redmine_attachment_path(
            &self.http.base_url,
            attachment.id,
            attachment.content_url.as_deref(),
        )?;
        let (content_type, content) = self.http.get_bytes(&path).await?;
        if attachment
            .filesize
            .is_some_and(|size| size != content.len() as u64)
        {
            bail!(
                "Redmine attachment {} の metadata filesize と取得本体のサイズが一致しません",
                attachment.id
            );
        }
        Ok(DownloadedRedmineAttachment {
            content_type,
            content,
        })
    }
}

#[derive(Clone)]
pub struct PortalApi {
    http: ApiClient,
}

impl PortalApi {
    pub fn from_env(max_retries: u32, retry_base_delay_ms: u64) -> Result<Self> {
        let base_url =
            std::env::var("PORTAL_BASE_URL").context("PORTAL_BASE_URL が設定されていません")?;
        let session_id = std::env::var("PORTAL_API_SESSION_ID")
            .context("PORTAL_API_SESSION_ID が設定されていません")?;
        Self::new(base_url, session_id, max_retries, retry_base_delay_ms)
    }

    pub fn new(
        base_url: String,
        session_id: String,
        max_retries: u32,
        retry_base_delay_ms: u64,
    ) -> Result<Self> {
        if base_url.trim().is_empty() {
            bail!("Portal base URL が空です");
        }
        if session_id.trim().is_empty() {
            bail!("Portal API session ID が空です");
        }
        Ok(Self {
            http: ApiClient::new(
                base_url,
                ApiAuthorization::PortalSession(session_id),
                max_retries,
                retry_base_delay_ms,
            )?,
        })
    }

    pub async fn fetch_comments(
        &self,
        form_id: FormId,
        answer_id: AnswerId,
    ) -> Result<Vec<PortalComment>> {
        self.http
            .get_json(
                &format!("/forms/{form_id}/answers/{answer_id}/comments"),
                &[],
            )
            .await
    }

    pub async fn find_answer_id(&self, form_id: FormId, redmine_issue_id: i64) -> Result<AnswerId> {
        let mut cursor: Option<String> = None;
        let mut found = None;
        loop {
            let mut query = vec![("limit", "100".to_owned())];
            if let Some(cursor_value) = &cursor {
                query.push(("cursor", cursor_value.clone()));
            }
            let page: PortalAnswerListPage = self
                .http
                .get_json(&format!("/forms/{form_id}/answers"), &query)
                .await?;
            for answer in page.items {
                if answer.redmine_issue_id != Some(redmine_issue_id) {
                    continue;
                }
                let answer_id: AnswerId = answer.id.into();
                if found.replace(answer_id).is_some() {
                    bail!(
                        "Portal に Redmine issue {} に対応する回答が複数あります",
                        redmine_issue_id
                    );
                }
            }
            match page.next_cursor {
                Some(next_cursor) => cursor = Some(next_cursor),
                None => break,
            }
        }
        found.with_context(|| {
            format!(
                "Portal に Redmine issue {} に対応する既存回答がありません",
                redmine_issue_id
            )
        })
    }

    pub async fn upload_attachments(
        &self,
        form_id: FormId,
        answer_id: AnswerId,
        comment_id: &str,
        uploads: Vec<PortalAttachmentUpload>,
    ) -> Result<()> {
        if uploads.is_empty() {
            return Ok(());
        }
        let parts = uploads
            .into_iter()
            .map(|upload| MultipartPart {
                file_name: upload.file_name,
                content_type: upload.content_type,
                content: upload.content,
            })
            .collect::<Vec<_>>();
        self.http
            .post_multipart(
                &format!("/forms/{form_id}/answers/{answer_id}/comments/{comment_id}/attachments"),
                &parts,
            )
            .await
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct PortalComment {
    pub id: String,
    pub redmine_journal_id: Option<i64>,
    #[serde(default)]
    pub attachments: Vec<PortalCommentAttachment>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PortalCommentAttachment {
    pub file_name: String,
    pub size: u64,
}

#[derive(Debug, Deserialize)]
struct PortalAnswerListPage {
    items: Vec<PortalAnswer>,
    next_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PortalAnswer {
    id: uuid::Uuid,
    redmine_issue_id: Option<i64>,
}

#[derive(Debug)]
pub struct PortalAttachmentUpload {
    pub file_name: String,
    pub content_type: String,
    pub content: Vec<u8>,
}

#[derive(Debug)]
pub struct DownloadedRedmineAttachment {
    pub content_type: String,
    pub content: Vec<u8>,
}

#[derive(Debug)]
pub struct AttachmentUploadCandidate {
    pub journal_id: i64,
    pub attachment: RedmineAttachment,
    pub content_type: String,
    pub content: Vec<u8>,
}

pub fn select_attachment_uploads(
    comments: &[PortalComment],
    candidates: Vec<AttachmentUploadCandidate>,
) -> Result<Vec<AttachmentUploadCandidate>> {
    let mut comments_by_journal = BTreeMap::new();
    let mut existing_attachment_counts = BTreeMap::new();
    for comment in comments {
        let Some(journal_id) = comment.redmine_journal_id else {
            continue;
        };
        if journal_id <= 0 {
            bail!("Portal comment の Redmine journal ID が不正です: {journal_id}");
        }
        if comment.id.is_empty() || uuid::Uuid::parse_str(&comment.id).is_err() {
            bail!("Portal comment ID が UUID ではありません: {:?}", comment.id);
        }
        if comments_by_journal.insert(journal_id, comment).is_some() {
            bail!(
                "Portal に同じ Redmine journal {} のコメントが複数あります",
                journal_id
            );
        }
        for attachment in &comment.attachments {
            *existing_attachment_counts
                .entry((journal_id, attachment.file_name.clone(), attachment.size))
                .or_insert(0_usize) += 1;
        }
    }

    let mut uploads = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        if !comments_by_journal.contains_key(&candidate.journal_id) {
            bail!(
                "Redmine journal {} に対応する Portal comment がありません",
                candidate.journal_id
            );
        }
        let size = candidate.content.len() as u64;
        if size > MAX_COMMENT_ATTACHMENT_SIZE {
            bail!(
                "Redmine attachment {} はサイズ上限 {} bytes を超えています",
                candidate.attachment.id,
                MAX_COMMENT_ATTACHMENT_SIZE
            );
        }
        if candidate
            .attachment
            .filesize
            .is_some_and(|expected| expected != size)
        {
            bail!(
                "Redmine attachment {} の metadata filesize と取得本体のサイズが一致しません",
                candidate.attachment.id
            );
        }
        let identity = (
            candidate.journal_id,
            candidate.attachment.filename.clone(),
            size,
        );
        if let Some(existing_count) = existing_attachment_counts.get_mut(&identity)
            && *existing_count > 0
        {
            *existing_count -= 1;
            continue;
        }
        uploads.push(candidate);
    }
    Ok(uploads)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssueStub {
    pub project_id: i64,
    pub id: i64,
    pub tracker_id: i64,
    pub tracker_name: String,
    pub relations: Vec<RedmineIssueRelation>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RedmineIssue {
    pub id: i64,
    pub project: NamedId,
    pub tracker: NamedId,
    pub subject: String,
    pub description: Option<String>,
    pub author: RedmineUser,
    pub created_on: String,
    pub status: NamedId,
    pub journals: Vec<RedmineJournal>,
    pub attachments: Vec<RedmineAttachment>,
    #[serde(default)]
    pub custom_fields: Vec<RedmineCustomField>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct NamedId {
    pub id: i64,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct RedmineUser {
    pub id: i64,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RedmineJournal {
    pub id: i64,
    pub user: Option<RedmineUser>,
    pub notes: Option<String>,
    pub created_on: String,
    #[serde(default)]
    pub details: Vec<RedmineJournalDetail>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct RedmineAttachment {
    pub id: i64,
    pub filename: String,
    #[serde(default)]
    pub filesize: Option<u64>,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub content_url: Option<String>,
    #[serde(default)]
    pub author: Option<RedmineUser>,
    #[serde(default)]
    pub created_on: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RedmineJournalDetail {
    pub property: String,
    pub name: String,
    #[serde(default)]
    pub new_value: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedmineAttachmentAssociation {
    pub journal_id: i64,
    pub attachment: RedmineAttachment,
}

#[derive(Debug)]
pub struct AttachmentAssociationResult {
    pub associations: Vec<RedmineAttachmentAssociation>,
    pub warnings: Vec<String>,
}

fn index_attachment_journal_candidates(
    journals: &[RedmineJournal],
) -> Result<BTreeMap<i64, Vec<i64>>> {
    let mut attachment_journals = BTreeMap::new();
    for journal in journals {
        if journal.id <= 0 {
            bail!("Redmine journal ID が不正です: {}", journal.id);
        }
        for detail in journal
            .details
            .iter()
            .filter(|detail| detail.property == "attachment")
        {
            let attachment_id = detail.name.parse::<i64>().with_context(|| {
                format!(
                    "Redmine journal {} の attachment detail name が ID ではありません: {:?}",
                    journal.id, detail.name
                )
            })?;
            if attachment_id <= 0 {
                bail!(
                    "Redmine journal {} の attachment ID が不正です: {}",
                    journal.id,
                    attachment_id
                );
            }
            let journal_ids = attachment_journals
                .entry(attachment_id)
                .or_insert_with(Vec::new);
            if !journal_ids.contains(&journal.id) {
                journal_ids.push(journal.id);
            }
        }
    }
    Ok(attachment_journals)
}

pub fn index_attachment_journals(journals: &[RedmineJournal]) -> Result<BTreeMap<i64, i64>> {
    index_attachment_journal_candidates(journals)?
        .into_iter()
        .map(|(attachment_id, journal_ids)| {
            let journal_id = *journal_ids.first().expect("journal IDs are non-empty");
            if journal_ids.len() > 1 {
                bail!(
                    "Redmine attachment {} が複数の journal に紐づいています: {} と {}",
                    attachment_id,
                    journal_ids[0],
                    journal_ids[1]
                );
            }
            Ok((attachment_id, journal_id))
        })
        .collect()
}

pub fn associate_attachments_with_fallback(
    journals: &[RedmineJournal],
    attachments: &[RedmineAttachment],
) -> Result<AttachmentAssociationResult> {
    let attachment_journals = index_attachment_journal_candidates(journals)?;
    let fallback_journal_ids = journals
        .iter()
        .filter(|journal| {
            !journal
                .notes
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
        })
        .map(|journal| journal.id)
        .collect::<Vec<_>>();
    let mut fallback_journal_index = 0;
    let mut attachment_ids = BTreeSet::new();
    let mut associations = Vec::with_capacity(attachments.len());
    let mut warnings = Vec::new();

    for attachment in attachments {
        if attachment.id <= 0 {
            bail!("Redmine attachment ID が不正です: {}", attachment.id);
        }
        if attachment.filename.is_empty() {
            bail!("Redmine attachment {} の filename が空です", attachment.id);
        }
        if !attachment_ids.insert(attachment.id) {
            bail!("Redmine attachment ID が重複しています: {}", attachment.id);
        }

        let candidate_journal_ids = attachment_journals
            .get(&attachment.id)
            .cloned()
            .unwrap_or_default();
        let non_empty_journal_ids = candidate_journal_ids
            .iter()
            .copied()
            .filter(|journal_id| {
                journals
                    .iter()
                    .find(|journal| journal.id == *journal_id)
                    .is_some_and(|journal| {
                        !journal
                            .notes
                            .as_deref()
                            .unwrap_or_default()
                            .trim()
                            .is_empty()
                    })
            })
            .collect::<Vec<_>>();
        let (journal_id, used_fallback) = match non_empty_journal_ids.first().copied() {
            Some(journal_id) => (journal_id, false),
            None => match (!fallback_journal_ids.is_empty()).then(|| {
                let journal_id =
                    fallback_journal_ids[fallback_journal_index % fallback_journal_ids.len()];
                fallback_journal_index += 1;
                journal_id
            }) {
                Some(journal_id) => (journal_id, true),
                None => {
                    warnings.push(format!(
                        "Redmine attachment {} は対応する notes 付き journal がないため Portal コメントへ移せません",
                        attachment.id
                    ));
                    continue;
                }
            },
        };

        if used_fallback && candidate_journal_ids.is_empty() {
            warnings.push(format!(
                "Redmine attachment {} に journal detail がないため notes 付き journal {} へ順番にフォールバックします",
                attachment.id, journal_id
            ));
        } else if used_fallback && candidate_journal_ids.len() > 1 {
            warnings.push(format!(
                "Redmine attachment {} が複数 journal ({}) に現れるため notes 付き journal {} へ順番にフォールバックします",
                attachment.id,
                candidate_journal_ids
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
                journal_id
            ));
        } else if used_fallback && non_empty_journal_ids.is_empty() {
            warnings.push(format!(
                "Redmine attachment {} が notes の空の journal {} に紐づくため notes 付き journal {} へ順番にフォールバックします",
                attachment.id, candidate_journal_ids[0], journal_id
            ));
        }

        if let Some(detail_filename) = candidate_journal_ids
            .iter()
            .filter_map(|candidate_journal_id| {
                journals
                    .iter()
                    .find(|journal| journal.id == *candidate_journal_id)
            })
            .flat_map(|journal| journal.details.iter())
            .find(|detail| {
                detail.property == "attachment" && detail.name == attachment.id.to_string()
            })
            .and_then(|detail| detail.new_value.as_deref())
            && detail_filename != attachment.filename
        {
            warnings.push(format!(
                "Redmine attachment {} の journal detail filename が一致しません: detail={detail_filename:?}, attachment={:?}",
                attachment.id, attachment.filename
            ));
        }

        associations.push(RedmineAttachmentAssociation {
            journal_id,
            attachment: attachment.clone(),
        });
    }

    for (attachment_id, journal_ids) in &attachment_journals {
        if !attachment_ids.contains(attachment_id) {
            warnings.push(format!(
                "Redmine journal {} が参照する attachment {} が issue detail の top-level attachments にありません",
                journal_ids[0], attachment_id
            ));
        }
    }

    Ok(AttachmentAssociationResult {
        associations,
        warnings,
    })
}

pub fn associate_attachments(
    journals: &[RedmineJournal],
    attachments: &[RedmineAttachment],
) -> Result<Vec<RedmineAttachmentAssociation>> {
    let attachment_journals = index_attachment_journals(journals)?;
    let mut attachment_ids = BTreeSet::new();
    let mut associations = Vec::with_capacity(attachments.len());
    for attachment in attachments {
        if attachment.id <= 0 {
            bail!("Redmine attachment ID が不正です: {}", attachment.id);
        }
        if attachment.filename.is_empty() {
            bail!("Redmine attachment {} の filename が空です", attachment.id);
        }
        if !attachment_ids.insert(attachment.id) {
            bail!("Redmine attachment ID が重複しています: {}", attachment.id);
        }
        let Some(&journal_id) = attachment_journals.get(&attachment.id) else {
            bail!(
                "Redmine attachment {} に対応する journal detail がありません",
                attachment.id
            );
        };
        let journal = journals
            .iter()
            .find(|journal| journal.id == journal_id)
            .expect("attachment journal index points to an existing journal");
        if journal
            .notes
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            bail!(
                "Redmine attachment {} が notes の空の journal {} に紐づいています。Portal コメントを作れません",
                attachment.id,
                journal_id
            );
        }
        if let Some(detail_filename) = journals
            .iter()
            .filter(|journal| journal.id == journal_id)
            .flat_map(|journal| journal.details.iter())
            .find(|detail| {
                detail.property == "attachment" && detail.name == attachment.id.to_string()
            })
            .and_then(|detail| detail.new_value.as_deref())
            && detail_filename != attachment.filename
        {
            bail!(
                "Redmine attachment {} の journal detail filename が一致しません: detail={detail_filename:?}, attachment={:?}",
                attachment.id,
                attachment.filename
            );
        }
        associations.push(RedmineAttachmentAssociation {
            journal_id,
            attachment: attachment.clone(),
        });
    }

    let top_level_ids = attachment_ids;
    if let Some((&attachment_id, &journal_id)) = attachment_journals
        .iter()
        .find(|(attachment_id, _)| !top_level_ids.contains(attachment_id))
    {
        bail!(
            "Redmine journal {} が参照する attachment {} が issue detail の top-level attachments にありません",
            journal_id,
            attachment_id
        );
    }
    Ok(associations)
}

#[derive(Clone, Debug, Deserialize)]
pub struct RedmineCustomField {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub value: Value,
}

#[derive(Debug, Deserialize)]
struct IssueStatusesResponse {
    issue_statuses: Vec<NamedId>,
}

#[derive(Debug, Deserialize)]
struct IssuePage {
    issues: Vec<IssueListItem>,
    total_count: u64,
    offset: u64,
}

#[derive(Debug, Deserialize)]
struct IssueListItem {
    id: i64,
    project: NamedId,
    tracker: NamedId,
    #[serde(default)]
    relations: Vec<RedmineIssueRelation>,
}

#[derive(Debug, Deserialize)]
struct IssueResponse {
    issue: RedmineIssue,
}

pub fn retry_delay(attempt: u32, base_delay_ms: u64) -> Duration {
    let multiplier = 1_u64.checked_shl(attempt.min(10)).unwrap_or(1 << 10);
    Duration::from_millis(base_delay_ms.saturating_mul(multiplier))
}

pub fn is_retryable_status(status: StatusCode) -> bool {
    status.is_server_error()
        || matches!(
            status,
            StatusCode::REQUEST_TIMEOUT
                | StatusCode::TOO_MANY_REQUESTS
                | StatusCode::SERVICE_UNAVAILABLE
        )
}

pub fn next_page_offset(
    current_offset: u64,
    total_count: u64,
    page_len: u64,
) -> Result<Option<u64>> {
    if page_len == 0 {
        if current_offset >= total_count {
            return Ok(None);
        }
        bail!(
            "Redmine pagination が空のページを返しました: offset={current_offset}, total={total_count}"
        );
    }
    if total_count == 0 {
        bail!("Redmine pagination の total_count が不正です");
    }

    let next_offset = current_offset
        .checked_add(page_len)
        .context("Redmine pagination の offset がオーバーフローしました")?;
    if next_offset >= total_count {
        Ok(None)
    } else if next_offset <= current_offset {
        bail!("Redmine pagination の offset が進みません");
    } else {
        Ok(Some(next_offset))
    }
}

fn redmine_attachment_path(
    base_url: &str,
    attachment_id: i64,
    content_url: Option<&str>,
) -> Result<String> {
    let Some(content_url) = content_url.filter(|url| !url.trim().is_empty()) else {
        return Ok(format!("/attachments/download/{attachment_id}"));
    };
    let base_url = reqwest::Url::parse(base_url).context("Redmine base URL が不正です")?;
    let content_url =
        reqwest::Url::parse(content_url).context("Redmine attachment の URL が不正です")?;
    if base_url.origin() != content_url.origin() {
        bail!(
            "Redmine attachment {} の URL が Redmine base URL と異なります",
            attachment_id
        );
    }
    Ok(content_url.to_string())
}

pub struct BuiltIssueInput {
    pub input: RedmineIssueInput,
    pub skipped_empty_journal_ids: Vec<i64>,
}

pub fn build_issue_input(
    issue: RedmineIssue,
    status: AnswerStatus,
    question_values: BTreeMap<String, String>,
) -> Result<BuiltIssueInput> {
    let issue_id = RedmineIssueId::try_new(issue.id)?;
    let author_display_name = redmine_author_display_name(&issue.author.name, &issue.custom_fields);
    let author = RedmineUserSnapshot::try_new(
        Some(positive_redmine_user_id(issue.author.id, "issue author")?),
        author_display_name,
    )?;
    let created_at = parse_timestamp(&issue.created_on, "issue created_on")?;
    let mut skipped_empty_journal_ids = Vec::new();
    let journals = issue
        .journals
        .into_iter()
        .filter_map(|mut journal| {
            let notes = journal.notes.take().unwrap_or_default();
            if notes.trim().is_empty() {
                skipped_empty_journal_ids.push(journal.id);
                return None;
            }
            Some((journal, notes))
        })
        .map(|(journal, notes)| {
            let user = journal.user.with_context(|| {
                format!("Redmine journal {} の author がありません", journal.id)
            })?;
            let author = RedmineUserSnapshot::try_new(
                Some(positive_redmine_user_id(user.id, "journal author")?),
                user.name,
            )?;
            let created_at = parse_timestamp(&journal.created_on, "journal created_on")?;
            RedmineJournalInput::new(journal.id, author, notes, created_at).map_err(Into::into)
        })
        .collect::<Result<Vec<_>>>()?;

    let input = RedmineIssueInput::new(
        issue_id,
        issue.subject,
        question_values,
        author,
        created_at,
        status,
        journals,
    )
    .map_err(anyhow::Error::from)?;

    Ok(BuiltIssueInput {
        input,
        skipped_empty_journal_ids,
    })
}

fn positive_redmine_user_id(id: i64, role: &str) -> Result<i64> {
    if id <= 0 {
        bail!("Redmine {role} ID が不正です: {id}");
    }
    Ok(id)
}

fn parse_timestamp(value: &str, field: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .with_context(|| format!("Redmine {field} の日時が不正です: {value:?}"))
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .and_then(|timestamp| {
            // Portal の既存 TIMESTAMP 列は小数秒を保持しないため、保存前に同じ精度へ
            // 正規化して再実行時の冪等性を保つ。
            timestamp
                .with_nanosecond(0)
                .context("Redmine の日時を秒精度へ正規化できません")
        })
}

fn truncate_for_error(value: &str) -> String {
    const MAX_ERROR_BODY_LENGTH: usize = 512;
    value.chars().take(MAX_ERROR_BODY_LENGTH).collect()
}

pub fn publication_for_tracker(tracker_id: i64) -> AnswerPublication {
    if tracker_id == PUBLIC_TRACKER_ID {
        AnswerPublication::PUBLIC
    } else {
        AnswerPublication::PRIVATE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_stops_at_total_count() {
        assert_eq!(next_page_offset(0, 3, 3).unwrap(), None);
        assert_eq!(next_page_offset(0, 4, 3).unwrap(), Some(3));
        assert_eq!(next_page_offset(3, 4, 1).unwrap(), None);
    }

    #[test]
    fn pagination_rejects_empty_page_before_total_count() {
        assert!(next_page_offset(0, 1, 0).is_err());
    }

    #[test]
    fn retry_delay_is_exponential_and_saturates_the_shift() {
        assert_eq!(retry_delay(0, 100), Duration::from_millis(100));
        assert_eq!(retry_delay(3, 100), Duration::from_millis(800));
        assert_eq!(retry_delay(20, 100), Duration::from_millis(102_400));
    }

    #[test]
    fn retryable_statuses_include_rate_limit_and_server_errors() {
        assert!(is_retryable_status(StatusCode::TOO_MANY_REQUESTS));
        assert!(is_retryable_status(StatusCode::INTERNAL_SERVER_ERROR));
        assert!(!is_retryable_status(StatusCode::BAD_REQUEST));
    }

    #[test]
    fn attachment_path_rejects_an_external_origin() {
        assert_eq!(
            redmine_attachment_path(
                "https://redmine.example.test",
                20,
                Some("https://redmine.example.test/attachments/download/20/file.txt")
            )
            .unwrap(),
            "https://redmine.example.test/attachments/download/20/file.txt"
        );
        assert!(
            redmine_attachment_path(
                "https://redmine.example.test",
                20,
                Some("https://attacker.example.test/file.txt")
            )
            .is_err()
        );
    }

    #[test]
    fn issue_detail_requires_the_journals_field() {
        let payload = serde_json::json!({
            "id": 1,
            "project": { "id": 1, "name": "project" },
            "tracker": { "id": 1, "name": "不具合" },
            "subject": "subject",
            "description": null,
            "author": { "id": 2, "name": "author" },
            "created_on": "2024-01-02T03:04:05Z",
            "status": { "id": 1, "name": "New" }
        });

        assert!(serde_json::from_value::<RedmineIssue>(payload).is_err());
    }

    #[test]
    fn issue_detail_requires_the_attachments_field() {
        let payload = serde_json::json!({
            "id": 1,
            "project": { "id": 1, "name": "project" },
            "tracker": { "id": 1, "name": "不具合" },
            "subject": "subject",
            "description": null,
            "author": { "id": 2, "name": "author" },
            "created_on": "2024-01-02T03:04:05Z",
            "status": { "id": 1, "name": "New" },
            "journals": [],
        });

        assert!(serde_json::from_value::<RedmineIssue>(payload).is_err());
    }

    #[test]
    fn build_issue_input_skips_empty_journals_and_normalizes_timestamps() {
        let issue = RedmineIssue {
            id: 1,
            project: NamedId {
                id: 1,
                name: "project".to_string(),
            },
            tracker: NamedId {
                id: 1,
                name: "不具合".to_string(),
            },
            subject: "subject".to_string(),
            description: None,
            author: RedmineUser {
                id: 2,
                name: "author".to_string(),
            },
            created_on: "2024-01-02T03:04:05.123456Z".to_string(),
            status: NamedId {
                id: 1,
                name: "New".to_string(),
            },
            journals: vec![
                RedmineJournal {
                    id: 10,
                    user: None,
                    notes: Some("   ".to_string()),
                    created_on: "not-a-date".to_string(),
                    details: Vec::new(),
                },
                RedmineJournal {
                    id: 11,
                    user: Some(RedmineUser {
                        id: 3,
                        name: "journal author".to_string(),
                    }),
                    notes: Some("notes".to_string()),
                    created_on: "2024-01-02T03:04:06.654321Z".to_string(),
                    details: Vec::new(),
                },
            ],
            attachments: Vec::new(),
            custom_fields: Vec::new(),
        };

        let built = build_issue_input(issue, AnswerStatus::IN_PROGRESS, BTreeMap::new()).unwrap();

        assert_eq!(built.skipped_empty_journal_ids, vec![10]);
        assert_eq!(
            parse_timestamp("2024-01-02T03:04:05.123456Z", "test")
                .unwrap()
                .timestamp_subsec_nanos(),
            0
        );
    }

    #[test]
    fn author_display_name_uses_custom_identity_for_bots_and_regular_authors() {
        let custom_fields = vec![
            RedmineCustomField {
                id: CONTACT_CUSTOM_FIELD_ID,
                name: "連絡先".to_string(),
                value: serde_json::json!(" contact@example.com "),
            },
            RedmineCustomField {
                id: ID_CUSTOM_FIELD_ID,
                name: "ID".to_string(),
                value: serde_json::json!(" discord-user "),
            },
        ];

        for original_name in ["なのです", "Observerをお助けするでござる", "regular author"]
        {
            assert_eq!(
                redmine_author_display_name(original_name, &custom_fields),
                "ID: discord-user / 連絡先: contact@example.com"
            );
        }

        assert_eq!(
            redmine_author_display_name(
                "fallback",
                &[RedmineCustomField {
                    id: ID_CUSTOM_FIELD_ID,
                    name: "ID".to_string(),
                    value: serde_json::json!("discord-user"),
                }]
            ),
            "discord-user"
        );
        assert_eq!(
            redmine_author_display_name(
                "fallback",
                &[RedmineCustomField {
                    id: CONTACT_CUSTOM_FIELD_ID,
                    name: "連絡先".to_string(),
                    value: serde_json::json!("contact@example.com"),
                }]
            ),
            "contact@example.com"
        );
    }

    #[test]
    fn author_display_name_ignores_unfilled_values_and_keeps_original_name() {
        for value in [
            "",
            "   ",
            "なし",
            "無し",
            "不明",
            "未回答",
            "未記入",
            "未入力",
            "?",
            "。",
        ] {
            assert_eq!(
                redmine_author_display_name(
                    "original author",
                    &[RedmineCustomField {
                        id: ID_CUSTOM_FIELD_ID,
                        name: "ID".to_string(),
                        value: serde_json::json!(value),
                    }]
                ),
                "original author"
            );
        }

        assert_eq!(
            redmine_author_display_name(
                "original author",
                &[
                    RedmineCustomField {
                        id: ID_CUSTOM_FIELD_ID,
                        name: "ID".to_string(),
                        value: serde_json::json!("なし"),
                    },
                    RedmineCustomField {
                        id: CONTACT_CUSTOM_FIELD_ID,
                        name: "連絡先".to_string(),
                        value: serde_json::json!(" contact@example.com "),
                    },
                ],
            ),
            "contact@example.com"
        );
    }

    #[test]
    fn checked_in_mapping_is_complete_and_visibility_is_tracker_based() {
        let config: Config =
            serde_json::from_str(include_str!("../config/redmine-import.json")).unwrap();
        config.validate().unwrap();

        assert_eq!(
            publication_for_tracker(PUBLIC_TRACKER_ID),
            AnswerPublication::PUBLIC
        );
        assert_eq!(publication_for_tracker(1), AnswerPublication::PRIVATE);
        assert_eq!(publication_for_tracker(19), AnswerPublication::PUBLIC);
    }

    #[test]
    fn custom_fields_are_mapped_or_appended_without_losing_values() {
        let config: Config =
            serde_json::from_str(include_str!("../config/redmine-import.json")).unwrap();
        let issue = RedmineIssue {
            id: 1,
            project: NamedId {
                id: 3,
                name: "project".to_string(),
            },
            tracker: NamedId {
                id: 6,
                name: "通報".to_string(),
            },
            subject: "summary".to_string(),
            description: Some("description".to_string()),
            author: RedmineUser {
                id: 2,
                name: "author".to_string(),
            },
            created_on: "2024-01-02T03:04:05Z".to_string(),
            status: NamedId {
                id: 1,
                name: "新規".to_string(),
            },
            journals: Vec::new(),
            attachments: Vec::new(),
            custom_fields: vec![
                RedmineCustomField {
                    id: 1,
                    name: "連絡先".to_string(),
                    value: serde_json::json!("contact@example.com"),
                },
                RedmineCustomField {
                    id: 3,
                    name: "違反者ID".to_string(),
                    value: serde_json::json!("target-player"),
                },
                RedmineCustomField {
                    id: 7,
                    name: "空の項目".to_string(),
                    value: serde_json::json!(""),
                },
            ],
        };

        let values = config.question_values_for(6, &issue).unwrap();

        assert_eq!(
            values.get("target_minecraft_id"),
            Some(&"target-player".to_string())
        );
        assert!(values["report_content"].contains("連絡先 (ID: 1): contact@example.com"));
        assert!(!values["report_content"].contains("違反者ID (ID: 3)"));
        assert!(!values["report_content"].contains("空の項目"));
    }

    #[test]
    fn idea_status_labels_are_only_configured_for_approved_and_rejected() {
        let config: Config =
            serde_json::from_str(include_str!("../config/redmine-import.json")).unwrap();

        assert_eq!(
            config.status_label_for(19, "承認"),
            Some("移行元ステータス: 承認")
        );
        assert_eq!(
            config.status_label_for(19, "却下"),
            Some("移行元ステータス: 却下")
        );
        assert_eq!(config.status_label_for(19, "新規"), None);
        assert_eq!(config.status_label_for(6, "承認"), None);
    }

    #[test]
    fn inquiry_classification_requires_an_explicit_or_high_confidence_marker() {
        assert_eq!(
            classify_inquiry("[user] Modの使用可否 Sodium", None),
            InquiryClassification::Mod
        );
        assert_eq!(
            classify_inquiry("その他の理由によりサーバーにログインできない", None),
            InquiryClassification::Login
        );
        assert_eq!(
            classify_inquiry("BAN解除申立", None),
            InquiryClassification::Appeal
        );
        assert_eq!(
            classify_inquiry(
                "その他お問い合わせ",
                Some("経験値のオーバーフローが発生しました")
            ),
            InquiryClassification::ExperienceOverflow
        );
        assert_eq!(
            classify_inquiry("ログインについての質問", Some("Modを使っています")),
            InquiryClassification::Generic
        );
        assert_eq!(
            classify_inquiry(
                "その他お問い合わせ",
                Some(
                    "経験値について相談したあと、これは今回の相談とは別の長い説明文であり、さらに別の話題としてオーバーフローという言葉を見ました"
                )
            ),
            InquiryClassification::Generic
        );
    }

    #[test]
    fn special_question_sources_keep_original_content_and_use_safe_fallbacks() {
        assert_eq!(
            extract_mod_name("[user] Modの使用可否 Sodium", "本文"),
            "Sodium"
        );
        assert_eq!(
            extract_mod_name("[user] Mod使用可否のお問い合わせ", "本文"),
            "（件名から特定できず）"
        );
        assert_eq!(
            first_url(
                "件名",
                "詳しくは https://example.com/mod を確認してください。"
            ),
            "https://example.com/mod"
        );
        assert_eq!(first_url("件名", "URLはありません"), "（URLの記載なし）");
        assert_eq!(
            extract_ipv4_addresses("接続元 192.168.0.10", "別のIP 8.8.8.8"),
            "192.168.0.10\n8.8.8.8"
        );
    }

    #[test]
    fn bug_type_defaults_to_in_game_and_relations_are_deduplicated_by_id() {
        let config: Config =
            serde_json::from_str(include_str!("../config/redmine-import.json")).unwrap();
        assert!(matches!(
            config
                .tracker_mappings
                .get(&9)
                .unwrap()
                .question_mappings
                .get("bug_type"),
            Some(QuestionValueSource::Static { value }) if value == "ゲーム内"
        ));

        let relation = RedmineIssueRelation {
            id: 1,
            issue_id: 10,
            issue_to_id: 11,
            relation_type: "relates".to_string(),
        };
        let reverse_relation = RedmineIssueRelation {
            id: 1,
            issue_id: 11,
            issue_to_id: 10,
            relation_type: "relates".to_string(),
        };
        let stubs = vec![
            IssueStub {
                project_id: 3,
                id: 10,
                tracker_id: 4,
                tracker_name: "お問い合わせ".to_string(),
                relations: vec![relation.clone()],
            },
            IssueStub {
                project_id: 3,
                id: 11,
                tracker_id: 9,
                tracker_name: "不具合報告".to_string(),
                relations: vec![reverse_relation],
            },
        ];
        assert_eq!(unique_issue_relations(&stubs).unwrap().len(), 1);
    }

    #[test]
    fn status_mapping_must_cover_exactly_the_redmine_statuses() {
        let config: Config =
            serde_json::from_str(include_str!("../config/redmine-import.json")).unwrap();
        let statuses = config
            .status_mapping
            .keys()
            .enumerate()
            .map(|(id, name)| NamedId {
                id: id as i64 + 1,
                name: name.clone(),
            })
            .collect::<Vec<_>>();

        assert!(config.validate_statuses(&statuses).is_ok());
        assert!(
            config
                .validate_statuses(&statuses[..statuses.len() - 1])
                .is_err()
        );
    }

    #[test]
    fn attachment_details_are_associated_with_non_empty_journals() {
        let journal = RedmineJournal {
            id: 10,
            user: None,
            notes: Some("comment".to_owned()),
            created_on: "2024-01-02T03:04:05Z".to_owned(),
            details: vec![RedmineJournalDetail {
                property: "attachment".to_owned(),
                name: "20".to_owned(),
                new_value: Some("file.txt".to_owned()),
            }],
        };
        let attachment = RedmineAttachment {
            id: 20,
            filename: "file.txt".to_owned(),
            filesize: Some(3),
            content_type: Some("text/plain".to_owned()),
            content_url: Some("https://example.test/file.txt".to_owned()),
            author: None,
            created_on: None,
        };

        let associations = associate_attachments(&[journal], &[attachment]).unwrap();
        assert_eq!(associations.len(), 1);
        assert_eq!(associations[0].journal_id, 10);
        assert_eq!(associations[0].attachment.id, 20);
    }

    #[test]
    fn attachment_association_rejects_empty_journals_and_unmapped_files() {
        let empty_journal = RedmineJournal {
            id: 10,
            user: None,
            notes: Some("  ".to_owned()),
            created_on: "2024-01-02T03:04:05Z".to_owned(),
            details: vec![RedmineJournalDetail {
                property: "attachment".to_owned(),
                name: "20".to_owned(),
                new_value: None,
            }],
        };
        let attachment = RedmineAttachment {
            id: 20,
            filename: "file.txt".to_owned(),
            filesize: None,
            content_type: None,
            content_url: None,
            author: None,
            created_on: None,
        };

        assert!(associate_attachments(&[empty_journal], &[attachment]).is_err());
        assert!(
            associate_attachments(
                &[],
                &[RedmineAttachment {
                    id: 20,
                    filename: "file.txt".to_owned(),
                    filesize: None,
                    content_type: None,
                    content_url: None,
                    author: None,
                    created_on: None,
                },]
            )
            .is_err()
        );
    }

    #[test]
    fn attachment_association_falls_back_across_non_empty_journals() {
        let result = associate_attachments_with_fallback(
            &[
                RedmineJournal {
                    id: 10,
                    user: None,
                    notes: Some("  ".to_owned()),
                    created_on: "2024-01-02T03:04:05Z".to_owned(),
                    details: Vec::new(),
                },
                RedmineJournal {
                    id: 11,
                    user: None,
                    notes: Some("comment".to_owned()),
                    created_on: "2024-01-02T03:04:06Z".to_owned(),
                    details: Vec::new(),
                },
                RedmineJournal {
                    id: 12,
                    user: None,
                    notes: Some("another comment".to_owned()),
                    created_on: "2024-01-02T03:04:07Z".to_owned(),
                    details: Vec::new(),
                },
            ],
            &[
                RedmineAttachment {
                    id: 20,
                    filename: "file.txt".to_owned(),
                    filesize: None,
                    content_type: None,
                    content_url: None,
                    author: None,
                    created_on: None,
                },
                RedmineAttachment {
                    id: 21,
                    filename: "another.txt".to_owned(),
                    filesize: None,
                    content_type: None,
                    content_url: None,
                    author: None,
                    created_on: None,
                },
            ],
        )
        .unwrap();

        assert_eq!(result.associations[0].journal_id, 11);
        assert_eq!(result.associations[1].journal_id, 12);
        assert_eq!(result.warnings.len(), 2);
    }

    #[test]
    fn attachment_selection_skips_existing_files_but_keeps_distinct_source_files() {
        let comments = vec![PortalComment {
            id: "00000000-0000-0000-0000-000000000001".to_owned(),
            redmine_journal_id: Some(10),
            attachments: vec![
                PortalCommentAttachment {
                    file_name: "already.txt".to_owned(),
                    size: 3,
                },
                PortalCommentAttachment {
                    file_name: "same-name.txt".to_owned(),
                    size: 3,
                },
            ],
        }];
        let candidate = |id: i64, filename: &str| AttachmentUploadCandidate {
            journal_id: 10,
            attachment: RedmineAttachment {
                id,
                filename: filename.to_owned(),
                filesize: Some(3),
                content_type: None,
                content_url: None,
                author: None,
                created_on: None,
            },
            content_type: "text/plain".to_owned(),
            content: b"abc".to_vec(),
        };

        let uploads = select_attachment_uploads(
            &comments,
            vec![
                candidate(20, "already.txt"),
                candidate(21, "same-name.txt"),
                candidate(22, "same-name.txt"),
            ],
        )
        .unwrap();

        assert_eq!(
            uploads
                .iter()
                .map(|upload| upload.attachment.id)
                .collect::<Vec<_>>(),
            vec![22]
        );
    }
}
