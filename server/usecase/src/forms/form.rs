use chrono::Utc;
use domain::{
    account::models::AccountUser,
    auth::Actor,
    form::models::{
        ActiveForm, AllowedUserGroups, AnswerAcceptancePeriod, AnswerSettings, AnswerVisibility,
        ArchivedForm, ArchivedFormPagePosition, DefaultAnswerTitle, DiscordWebhookUrl,
        FormDescription, FormId, FormLabel, FormLabelAssignment, FormLabelId, FormPagePosition,
        FormSettings, FormTitle, Question, QuestionSet, Visibility,
    },
    pagination::{Page, PageLimit, PageRequest},
    repository::{
        form::{
            active_form_repository::ActiveFormRepository,
            answer_entry_repository::AnswerEntryRepository,
            archived_form_repository::ArchivedFormRepository,
            form_label_repository::FormLabelRepository,
        },
        notification_repository::NotificationRepository,
        user_repository::UserRepository,
    },
    types::authorization_guard::{AuthorizationGuard, Create},
};
use errors::{
    Error,
    domain::DomainError,
    usecase::UseCaseError::{FormNotFound, LabelNotFound, UserGroupNotFound, UserNotFound},
};
use std::collections::{BTreeSet, HashMap};
use types::non_empty_string::NonEmptyString;
use types::non_empty_vec::NonEmptyVec;

use crate::{
    application_event::{
        ApplicationActor, ApplicationEvent, ApplicationEventPublisher, EventDetail,
    },
    models::{ActiveFormWithLabels, ArchivedFormDetails, UpsertQuestionInput},
};

pub struct FormUseCase<
    'a,
    FormRepo: ActiveFormRepository,
    ArchivedFormRepo: ArchivedFormRepository,
    NotificationRepo: NotificationRepository,
    FormLabelRepo: FormLabelRepository,
    AnswerEntryRepo: AnswerEntryRepository,
    UserRepo: UserRepository,
> {
    pub active_form_repository: &'a FormRepo,
    pub archived_form_repository: &'a ArchivedFormRepo,
    pub notification_repository: &'a NotificationRepo,
    pub form_label_repository: &'a FormLabelRepo,
    pub answer_entry_repository: &'a AnswerEntryRepo,
    pub user_repository: &'a UserRepo,
    pub application_event_publisher: Option<&'a dyn ApplicationEventPublisher>,
}

impl<
    R1: ActiveFormRepository,
    R2: ArchivedFormRepository,
    R3: NotificationRepository,
    R4: FormLabelRepository,
    R5: AnswerEntryRepository,
    R6: UserRepository,
> FormUseCase<'_, R1, R2, R3, R4, R5, R6>
{
    async fn validate_allowed_user_groups(
        &self,
        actor: &Actor,
        groups: &AllowedUserGroups,
    ) -> Result<(), Error> {
        for group_id in groups.as_slice() {
            self.user_repository
                .find_user_group(*group_id)
                .await?
                .ok_or(Error::from(UserGroupNotFound))?
                .try_read(actor.clone())?;
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_form(
        &self,
        title: FormTitle,
        description: FormDescription,
        questions: NonEmptyVec<Question>,
        discord_webhook_url: Option<DiscordWebhookUrl>,
        visibility: Option<Visibility>,
        allowed_user_groups: Option<AllowedUserGroups>,
        allow_temporary_answers: Option<bool>,
        answer_visibility: Option<AnswerVisibility>,
        answer_groups: Option<AllowedUserGroups>,
        acceptance_period: Option<AnswerAcceptancePeriod>,
        default_answer_title: Option<DefaultAnswerTitle>,
        user: &AccountUser,
    ) -> Result<ActiveForm, Error> {
        let user_as_user = Actor::from(user.clone());
        if let Some(groups) = &allowed_user_groups {
            self.validate_allowed_user_groups(&user_as_user, groups)
                .await?;
        }
        if let Some(groups) = &answer_groups {
            self.validate_allowed_user_groups(&user_as_user, groups)
                .await?;
        }

        let form_settings = FormSettings::new();
        let form_settings = match discord_webhook_url {
            Some(discord_webhook_url) => {
                form_settings.change_discord_webhook_url(discord_webhook_url)
            }
            None => form_settings,
        };
        let form_settings = match visibility {
            Some(visibility) => form_settings.change_visibility(visibility),
            None => form_settings,
        };
        let form_settings = match allowed_user_groups {
            Some(allowed_user_groups) => {
                form_settings.change_allowed_user_groups(allowed_user_groups)
            }
            None => form_settings,
        };

        let answer_settings = AnswerSettings::default();
        let answer_settings = match allow_temporary_answers {
            Some(allow) => answer_settings.change_allow_temporary_answers(allow),
            None => answer_settings,
        };
        let answer_settings = match answer_visibility {
            Some(visibility) => answer_settings.change_visibility(visibility),
            None => answer_settings,
        };
        let answer_settings = match answer_groups {
            Some(groups) => answer_settings.change_answer_groups(groups),
            None => answer_settings,
        };
        let answer_settings = match acceptance_period {
            Some(acceptance_period) => answer_settings.change_acceptance_period(acceptance_period),
            None => answer_settings,
        };
        let answer_settings = match default_answer_title {
            Some(default_answer_title) => {
                answer_settings.change_default_answer_title(default_answer_title)
            }
            None => answer_settings,
        };

        let form = ActiveForm::new(
            title,
            description,
            QuestionSet::try_new(questions).map_err(Error::from)?,
        )
        .change_settings(form_settings)
        .change_answer_settings(answer_settings);
        let form_id = *form.id();

        self.active_form_repository
            .create(
                user,
                AuthorizationGuard::<_, Create>::from(form).try_create(user_as_user.clone())?,
            )
            .await?;

        let created_form = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?
            .try_read(user_as_user.clone())
            .map(|form| form.into_inner())
            .map_err(Error::from)?;

        if let Some(publisher) = self.application_event_publisher {
            publisher.publish(ApplicationEvent::FormCreated {
                actor: ApplicationActor::from(user),
                form_id: form_id.to_string(),
                form_title: created_form.title().as_str().to_owned(),
                details: form_creation_details(&created_form),
            });
        }

        Ok(created_form)
    }

    /// `actor` が参照可能なフォームのリストを取得する
    pub async fn form_list(
        &self,
        actor: &Actor,
        request: PageRequest<FormPagePosition>,
    ) -> Result<Page<(ActiveForm, Vec<FormLabel>), FormPagePosition>, Error> {
        let page = self.active_form_repository.list(request).await?;
        let (forms, next) = page.into_parts();
        let forms = forms
            .into_iter()
            .flat_map(|form| form.try_read(actor.clone()).map(|form| form.into_inner()))
            .collect::<Vec<_>>();

        let form_labels = futures::future::try_join_all(forms.iter().map(|form| {
            self.form_label_repository
                .fetch_labels_by_form_id(*form.id())
        }))
        .await?;
        let forms_with_labels = forms
            .into_iter()
            .zip(form_labels)
            .map(|(form, labels)| {
                Ok::<_, Error>((
                    form,
                    labels
                        .into_iter()
                        .map(|guard| {
                            guard
                                .try_read(actor.clone())
                                .map(|label| label.into_inner())
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Page::new(forms_with_labels, next))
    }

    pub async fn get_form(
        &self,
        actor: &Actor,
        form_id: FormId,
    ) -> Result<ActiveFormWithLabels, Error> {
        let form = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?
            .try_read(actor.clone())?
            .into_inner();
        let labels = self
            .form_label_repository
            .fetch_labels_by_form_id(form_id)
            .await?
            .into_iter()
            .map(|label| {
                label
                    .try_read(actor.clone())
                    .map(|label| label.into_inner())
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ActiveFormWithLabels { form, labels })
    }

    pub async fn archived_form_list(
        &self,
        actor: &AccountUser,
        request: PageRequest<ArchivedFormPagePosition>,
        query: Option<String>,
    ) -> Result<Page<ArchivedFormDetails, ArchivedFormPagePosition>, Error> {
        let actor_user = Actor::from(actor.clone());
        let page = self.archived_form_repository.list(request, query).await?;
        let (forms, next) = page.into_parts();
        let forms = forms
            .into_iter()
            .flat_map(|form| {
                form.try_read(actor_user.clone())
                    .map(|form| form.into_inner())
            })
            .collect::<Vec<_>>();

        let form_labels = futures::future::try_join_all(forms.iter().map(|form| {
            self.form_label_repository
                .fetch_labels_by_form_id(form.form().id().to_owned())
        }))
        .await?;

        let forms_with_labels = forms
            .into_iter()
            .zip(form_labels)
            .map(|(form, labels)| {
                let actor_user = actor_user.clone();
                async move {
                    let archived_by = self
                        .user_repository
                        .find_by(form.archived_by().into_inner())
                        .await?
                        .ok_or(Error::from(UserNotFound))?
                        .try_read(actor_user.clone())?
                        .into_inner();
                    Ok::<_, Error>(ArchivedFormDetails {
                        archived_by,
                        form,
                        labels: labels
                            .into_iter()
                            .map(|label| {
                                label
                                    .try_read(actor_user.clone())
                                    .map(|label| label.into_inner())
                            })
                            .collect::<Result<Vec<_>, _>>()?,
                    })
                }
            })
            .collect::<Vec<_>>();

        let forms_with_labels = futures::future::try_join_all(forms_with_labels).await?;
        Ok(Page::new(forms_with_labels, next))
    }

    pub async fn get_archived_form(
        &self,
        actor: &AccountUser,
        form_id: FormId,
    ) -> Result<ArchivedFormDetails, Error> {
        let actor_user = Actor::from(actor.clone());
        let form = self
            .archived_form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?
            .try_read(actor_user.clone())?
            .into_inner();
        let labels = self
            .form_label_repository
            .fetch_labels_by_form_id(form.form().id().to_owned())
            .await?
            .into_iter()
            .map(|label| {
                label
                    .try_read(actor_user.clone())
                    .map(|label| label.into_inner())
            })
            .collect::<Result<Vec<_>, _>>()?;

        let archived_by = self
            .user_repository
            .find_by(form.archived_by().into_inner())
            .await?
            .ok_or(Error::from(UserNotFound))?
            .try_read(actor_user.clone())?
            .into_inner();

        Ok(ArchivedFormDetails {
            form,
            archived_by,
            labels,
        })
    }

    pub async fn archive_form(
        &self,
        actor: &AccountUser,
        form_id: FormId,
    ) -> Result<ArchivedForm, Error> {
        let actor_user = Actor::from(actor.clone());
        let form = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?;
        let form = form
            .try_read(actor_user.clone())?
            .into_inner()
            .archive(Utc::now(), *actor.id());
        let archived_form = self
            .archived_form_repository
            .archive(AuthorizationGuard::<_, Create>::from(form).try_create(actor_user.clone())?)
            .await?;
        let archived_form = archived_form
            .try_read(actor_user.clone())
            .map(|form| form.into_inner())
            .map_err(Error::from)?;
        if let Some(publisher) = self.application_event_publisher {
            publisher.publish(ApplicationEvent::FormArchived {
                actor: ApplicationActor::from(actor),
                form_id: form_id.to_string(),
                form_title: archived_form.form().title().as_str().to_owned(),
            });
        }

        Ok(archived_form)
    }

    pub async fn restore_form(&self, actor: &AccountUser, form_id: FormId) -> Result<(), Error> {
        let form = self
            .archived_form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?
            .try_read(Actor::from(actor.clone()))?;

        let form_title = form.value().form().title().as_str().to_owned();
        self.archived_form_repository
            .restore(form.try_into_update()?)
            .await?;
        if let Some(publisher) = self.application_event_publisher {
            publisher.publish(ApplicationEvent::FormRestored {
                actor: ApplicationActor::from(actor),
                form_id: form_id.to_string(),
                form_title,
            });
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_form(
        &self,
        actor: &AccountUser,
        form_id: FormId,
        title: Option<FormTitle>,
        description: Option<FormDescription>,
        acceptance_period: Option<AnswerAcceptancePeriod>,
        discord_webhook_url: Option<DiscordWebhookUrl>,
        default_answer_title: Option<DefaultAnswerTitle>,
        visibility: Option<Visibility>,
        allowed_user_groups: Option<AllowedUserGroups>,
        allow_temporary_answers: Option<bool>,
        answer_visibility: Option<AnswerVisibility>,
        answer_groups: Option<AllowedUserGroups>,
        questions: Option<Vec<UpsertQuestionInput>>,
        label_ids: Option<Vec<FormLabelId>>,
    ) -> Result<(ActiveForm, Vec<FormLabel>), Error> {
        let actor_user = Actor::from(actor.clone());
        if let Some(groups) = &allowed_user_groups {
            self.validate_allowed_user_groups(&actor_user, groups)
                .await?;
        }
        if let Some(groups) = &answer_groups {
            self.validate_allowed_user_groups(&actor_user, groups)
                .await?;
        }

        let current_form = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?;
        let current_form_read = current_form.try_read(actor_user.clone())?;
        let form_before_update = current_form_read.value().clone();
        let current_questions = current_form_read.questions().as_slice().to_vec();

        if let Some(questions) = &questions {
            let existing_question_ids = current_questions
                .iter()
                .map(|question| question.id().into_inner())
                .collect::<BTreeSet<_>>();
            if let Some(invalid_id) = questions
                .iter()
                .filter_map(|question| question.original_id.map(|id| id.into_inner()))
                .find(|id| !existing_question_ids.contains(id))
            {
                return Err(DomainError::InvalidEntity {
                    message: format!("question id {} does not belong to the form", invalid_id),
                }
                .into());
            }

            if !self
                .answer_entry_repository
                .list_by_form(
                    &current_form_read,
                    PageRequest::first(PageLimit::default_limit()),
                )
                .await?
                .items()
                .is_empty()
            {
                validate_answered_form_question_update(&current_questions, questions.as_slice())?;
            }
        }

        let label_ids = match label_ids {
            Some(label_ids) => {
                let label_ids = FormLabelAssignment::try_new(label_ids)?;
                let labels = self
                    .form_label_repository
                    .fetch_labels_by_ids(label_ids.as_slice().to_vec())
                    .await?;
                if labels.len() != label_ids.as_slice().len() {
                    return Err(Error::from(LabelNotFound));
                }
                Some(label_ids)
            }
            None => None,
        };

        let current_form = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?;

        let updated_form = current_form.into_update().map(|form| {
            let current_settings = form.settings().to_owned();
            let updated_settings = match visibility {
                None => current_settings,
                Some(visibility) => current_settings.change_visibility(visibility),
            };
            let updated_settings = match allowed_user_groups {
                None => updated_settings,
                Some(groups) => updated_settings.change_allowed_user_groups(groups),
            };
            let updated_settings = match discord_webhook_url {
                None => updated_settings,
                Some(discord_webhook_url) => {
                    updated_settings.change_discord_webhook_url(discord_webhook_url)
                }
            };

            let updated_answer_settings = form.answer_settings().to_owned();
            let updated_answer_settings = match answer_visibility {
                None => updated_answer_settings,
                Some(v) => updated_answer_settings.change_visibility(v),
            };
            let updated_answer_settings = match answer_groups {
                None => updated_answer_settings,
                Some(groups) => updated_answer_settings.change_answer_groups(groups),
            };
            let updated_answer_settings = match default_answer_title {
                None => updated_answer_settings,
                Some(t) => updated_answer_settings.change_default_answer_title(t),
            };
            let updated_answer_settings = match acceptance_period {
                None => updated_answer_settings,
                Some(p) => updated_answer_settings.change_acceptance_period(p),
            };
            let updated_answer_settings = match allow_temporary_answers {
                None => updated_answer_settings,
                Some(a) => updated_answer_settings.change_allow_temporary_answers(a),
            };

            let updated_form = match title {
                None => form,
                Some(title) => form.change_title(title),
            };
            let updated_form = match description {
                None => updated_form,
                Some(description) => updated_form.change_description(description),
            };
            updated_form
                .change_settings(updated_settings)
                .change_answer_settings(updated_answer_settings)
        });

        let updated_form = match label_ids {
            Some(label_ids) => updated_form.map(|form| form.replace_label_ids(label_ids)),
            None => updated_form,
        };

        let updated_form = match questions {
            Some(questions) => {
                let current_by_id = current_questions
                    .iter()
                    .cloned()
                    .map(|question| (question.id().into_inner(), question))
                    .collect::<HashMap<_, _>>();
                let questions = questions
                    .into_iter()
                    .map(|question| match question.original_id {
                        Some(original_id) => current_by_id
                            .get(&original_id.into_inner())
                            .cloned()
                            .ok_or_else(|| DomainError::InvalidEntity {
                                message: format!(
                                    "question id {} does not belong to the form",
                                    original_id
                                ),
                            })
                            .and_then(|current_question| {
                                current_question.update_preserving_id(question.question)
                            }),
                        None => Ok(question.question),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let questions = NonEmptyVec::try_new(questions).map_err(Error::from)?;
                updated_form.map(|form| {
                    form.change_questions(QuestionSet::try_new(questions).expect("validated"))
                })
            }
            None => updated_form,
        };

        self.active_form_repository
            .update_form(actor, updated_form.try_update(actor_user.clone())?)
            .await?;

        let updated_form = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?
            .try_read(actor_user.clone())
            .map(|form| form.into_inner())
            .map_err(|_| Error::from(FormNotFound))?;

        let label_guards = self
            .form_label_repository
            .fetch_labels_by_form_id(form_id)
            .await?;
        let labels = label_guards
            .into_iter()
            .map(|label| {
                label
                    .try_read(actor_user.clone())
                    .map(|label| label.into_inner())
            })
            .collect::<Result<Vec<_>, _>>()?;

        let changes = form_update_details(&form_before_update, &updated_form);
        if !changes.is_empty()
            && let Some(publisher) = self.application_event_publisher
        {
            publisher.publish(ApplicationEvent::FormUpdated {
                actor: ApplicationActor::from(actor),
                form_id: form_id.to_string(),
                form_title: updated_form.title().as_str().to_owned(),
                changes,
            });
        }

        Ok((updated_form, labels))
    }
}

fn form_creation_details(form: &ActiveForm) -> Vec<EventDetail> {
    vec![
        EventDetail::new("説明", form.description().to_owned().into_inner()),
        EventDetail::new("フォーム公開範囲", form.settings().visibility().to_string()),
        EventDetail::new(
            "フォーム閲覧グループ",
            format_groups(form.settings().allowed_user_groups()),
        ),
        EventDetail::new(
            "フォーム別 Discord 通知",
            format_form_webhook_status(form.settings()),
        ),
        EventDetail::new(
            "回答公開範囲",
            form.answer_settings().visibility().to_string(),
        ),
        EventDetail::new(
            "回答可能グループ",
            format_groups(form.answer_settings().answer_groups()),
        ),
        EventDetail::new(
            "回答受付期間",
            format_acceptance_period(form.answer_settings().acceptance_period()),
        ),
        EventDetail::new(
            "回答の既定タイトル",
            format_default_answer_title(form.answer_settings().default_answer_title()),
        ),
        EventDetail::new(
            "匿名回答",
            format_allowed(*form.answer_settings().allow_temporary_answers()),
        ),
    ]
    .into_iter()
    .chain(question_details(form.questions().as_slice()))
    .collect()
}

fn form_update_details(before: &ActiveForm, after: &ActiveForm) -> Vec<EventDetail> {
    [
        (before.title() != after.title())
            .then(|| EventDetail::new("タイトル", after.title().as_str())),
        (before.description() != after.description())
            .then(|| EventDetail::new("説明", after.description().to_owned().into_inner())),
        (before.settings().visibility() != after.settings().visibility()).then(|| {
            EventDetail::new(
                "フォーム公開範囲",
                after.settings().visibility().to_string(),
            )
        }),
        (before.settings().allowed_user_groups() != after.settings().allowed_user_groups()).then(
            || {
                EventDetail::new(
                    "フォーム閲覧グループ",
                    format_groups(after.settings().allowed_user_groups()),
                )
            },
        ),
        (before.settings().discord_webhook_url(&Actor::System).ok()
            != after.settings().discord_webhook_url(&Actor::System).ok())
        .then(|| {
            EventDetail::new(
                "フォーム別 Discord 通知",
                format_form_webhook_status(after.settings()),
            )
        }),
        (before.answer_settings().visibility() != after.answer_settings().visibility()).then(
            || {
                EventDetail::new(
                    "回答公開範囲",
                    after.answer_settings().visibility().to_string(),
                )
            },
        ),
        (before.answer_settings().answer_groups() != after.answer_settings().answer_groups()).then(
            || {
                EventDetail::new(
                    "回答可能グループ",
                    format_groups(after.answer_settings().answer_groups()),
                )
            },
        ),
        (before.answer_settings().acceptance_period()
            != after.answer_settings().acceptance_period())
        .then(|| {
            EventDetail::new(
                "回答受付期間",
                format_acceptance_period(after.answer_settings().acceptance_period()),
            )
        }),
        (before.answer_settings().default_answer_title()
            != after.answer_settings().default_answer_title())
        .then(|| {
            EventDetail::new(
                "回答の既定タイトル",
                format_default_answer_title(after.answer_settings().default_answer_title()),
            )
        }),
        (before.answer_settings().allow_temporary_answers()
            != after.answer_settings().allow_temporary_answers())
        .then(|| {
            EventDetail::new(
                "匿名回答",
                format_allowed(*after.answer_settings().allow_temporary_answers()),
            )
        }),
    ]
    .into_iter()
    .flatten()
    .chain(
        (before.questions() != after.questions())
            .then(|| question_details(after.questions().as_slice()))
            .into_iter()
            .flatten(),
    )
    .collect()
}

fn format_groups(groups: &AllowedUserGroups) -> String {
    match groups.as_slice() {
        [] => "制限なし".to_string(),
        groups => groups
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", "),
    }
}

fn format_form_webhook_status(settings: &FormSettings) -> &'static str {
    match settings
        .discord_webhook_url(&Actor::System)
        .ok()
        .cloned()
        .and_then(DiscordWebhookUrl::into_inner)
    {
        Some(_) => "有効 (URLは非表示)",
        None => "無効",
    }
}

fn format_acceptance_period(period: &AnswerAcceptancePeriod) -> String {
    let start = period
        .start_at()
        .map(|value| value.to_rfc3339())
        .unwrap_or_else(|| "開始指定なし".to_string());
    let end = period
        .end_at()
        .map(|value| value.to_rfc3339())
        .unwrap_or_else(|| "終了指定なし".to_string());
    format!("{start} ～ {end}")
}

fn format_default_answer_title(title: &DefaultAnswerTitle) -> String {
    title
        .to_owned()
        .into_inner()
        .map(NonEmptyString::into_inner)
        .unwrap_or_else(|| "未設定".to_string())
}

fn format_allowed(allowed: bool) -> &'static str {
    if allowed { "許可" } else { "不許可" }
}

fn question_details(questions: &[Question]) -> impl Iterator<Item = EventDetail> + '_ {
    questions.iter().map(|question| {
        let choices = question
            .choices()
            .map(|choices| {
                choices
                    .iter()
                    .map(|choice| choice.label.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .filter(|choices| !choices.is_empty());
        let value = [
            Some(format!("種別: {}", question.question_type())),
            Some(format!(
                "必須: {}",
                if question.is_required() {
                    "はい"
                } else {
                    "いいえ"
                }
            )),
            Some(format!("テンプレートキー: {}", question.template_key())),
            choices.map(|choices| format!("選択肢: {choices}")),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n");
        EventDetail::new(format!("質問: {}", question.title().as_str()), value)
    })
}

fn validate_answered_form_question_update(
    current_questions: &[Question],
    updated_questions: &[UpsertQuestionInput],
) -> Result<(), Error> {
    let current_by_id = current_questions
        .iter()
        .map(|question| (question.id().into_inner(), question))
        .collect::<HashMap<_, _>>();
    let updated_by_id = updated_questions
        .iter()
        .map(|question| {
            (
                question
                    .original_id
                    .unwrap_or_else(|| question.question.id())
                    .into_inner(),
                &question.question,
            )
        })
        .collect::<HashMap<_, _>>();

    if let Some(error) = current_questions
        .iter()
        .map(|current_question| (current_question.id().into_inner(), current_question))
        .find_map(|(current_id, current_question)| {
            let updated_question =
                updated_by_id
                    .get(&current_id)
                    .ok_or_else(|| DomainError::InvalidEntity {
                        message: format!(
                            "cannot delete question {} from a form that already has answers",
                            current_question.template_key().as_str()
                        ),
                    });

            updated_question
                .and_then(|updated_question| {
                    (current_question.template_key() == updated_question.template_key())
                        .then_some(updated_question)
                        .ok_or_else(|| DomainError::InvalidEntity {
                            message: format!(
                                "cannot change template_key for answered question {}",
                                current_question.template_key().as_str()
                            ),
                        })
                })
                .and_then(|updated_question| {
                    (current_question.question_type() == updated_question.question_type())
                        .then_some((current_question, updated_question))
                        .ok_or_else(|| DomainError::InvalidEntity {
                            message: format!(
                                "cannot change question_type for answered question {}",
                                current_question.template_key().as_str()
                            ),
                        })
                })
                .and_then(|(current_question, updated_question)| {
                    let current_choice_ids = current_question
                        .choices()
                        .into_iter()
                        .flat_map(|choices| {
                            choices
                                .iter()
                                .filter_map(|choice| choice.id.map(|id| id.into_inner()))
                        })
                        .collect::<BTreeSet<_>>();
                    let updated_choice_ids = updated_question
                        .choices()
                        .into_iter()
                        .flat_map(|choices| {
                            choices
                                .iter()
                                .filter_map(|choice| choice.id.map(|id| id.into_inner()))
                        })
                        .collect::<BTreeSet<_>>();

                    current_choice_ids
                        .into_iter()
                        .find(|choice_id| !updated_choice_ids.contains(choice_id))
                        .map(|choice_id| DomainError::InvalidEntity {
                            message: format!(
                                "cannot delete choice {} from answered question {}",
                                choice_id,
                                current_question.template_key().as_str()
                            ),
                        })
                        .map_or(Ok(()), Err)
                })
                .err()
        })
    {
        return Err(error.into());
    }

    if let Some(error) = updated_questions
        .iter()
        .filter_map(|updated_question| {
            updated_question
                .original_id
                .map(|id| (id.into_inner(), &updated_question.question))
        })
        .find_map(|(updated_id, updated_question)| {
            current_by_id
                .get(&updated_id)
                .ok_or_else(|| DomainError::InvalidEntity {
                    message: format!("question id {} does not belong to the form", updated_id),
                })
                .and_then(|current_question| {
                    let current_choice_ids = current_question
                        .choices()
                        .into_iter()
                        .flat_map(|choices| {
                            choices
                                .iter()
                                .filter_map(|choice| choice.id.map(|id| id.into_inner()))
                        })
                        .collect::<BTreeSet<_>>();

                    updated_question
                        .choices()
                        .into_iter()
                        .flat_map(|choices| {
                            choices
                                .iter()
                                .filter_map(|choice| choice.id.map(|id| id.into_inner()))
                        })
                        .find(|choice_id| !current_choice_ids.contains(choice_id))
                        .map(|choice_id| DomainError::InvalidEntity {
                            message: format!(
                                "cannot regenerate choice id {} for answered question {}",
                                choice_id,
                                updated_question.template_key().as_str()
                            ),
                        })
                        .map_or(Ok(()), Err)
                })
                .err()
        })
    {
        return Err(error.into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::repositories::FormUseCaseTestRepositories;
    use domain::{
        account::models::{AccountUser, Role},
        form::{
            models::{
                ActiveForm, FormDescription, FormLabelAssignment, FormMeta, FormSettings, FormTitle,
            },
            question::{QuestionId, QuestionSet, QuestionType},
        },
    };
    use std::sync::Mutex;
    use types::non_empty_vec::NonEmptyVec;
    use uuid::Uuid;

    #[derive(Default)]
    struct RecordingPublisher(Mutex<Vec<ApplicationEvent>>);

    impl ApplicationEventPublisher for RecordingPublisher {
        fn publish(&self, event: ApplicationEvent) {
            self.0.lock().unwrap().push(event);
        }
    }

    impl RecordingPublisher {
        fn events(&self) -> Vec<ApplicationEvent> {
            self.0.lock().unwrap().clone()
        }
    }

    fn admin_user() -> AccountUser {
        AccountUser::new("admin".to_string(), Uuid::nil().into(), Role::Administrator)
    }

    fn sample_form(form_id: FormId) -> ActiveForm {
        let questions = QuestionSet::try_new(
            NonEmptyVec::try_new(vec![text_question(
                QuestionId::from(Uuid::new_v4()),
                0,
                "body",
            )])
            .unwrap(),
        )
        .unwrap();
        unsafe {
            ActiveForm::from_raw_parts(
                form_id,
                FormTitle::new("Form".to_string().try_into().unwrap()),
                FormDescription::new("description".to_string()),
                FormMeta::new(),
                FormSettings::new(),
                AnswerSettings::default(),
                questions,
                FormLabelAssignment::empty(),
            )
        }
    }

    fn text_question(question_id: QuestionId, position: u16, template_key: &str) -> Question {
        unsafe {
            Question::from_raw_parts(
                question_id,
                template_key.to_string().try_into().unwrap(),
                position,
                template_key.to_string().try_into().unwrap(),
                None,
                QuestionType::Text,
                None,
                true,
            )
            .unwrap()
        }
    }

    #[tokio::test]
    async fn create_form_always_creates_questions_and_returns_them() {
        let user = admin_user();
        let input_questions = NonEmptyVec::try_new(vec![
            Question::new_text(
                "body".to_string().try_into().unwrap(),
                0,
                "Body".to_string().try_into().unwrap(),
                None,
                true,
            )
            .unwrap(),
        ])
        .unwrap();

        let repositories = FormUseCaseTestRepositories::default();
        let publisher = RecordingPublisher::default();
        let usecase = FormUseCase {
            application_event_publisher: Some(&publisher),
            ..repositories.form_use_case()
        };

        let created_form = usecase
            .create_form(
                FormTitle::new("Form".to_string().try_into().unwrap()),
                FormDescription::new("description".to_string()),
                input_questions,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                &user,
            )
            .await
            .unwrap();

        assert_eq!(created_form.questions().as_slice().len(), 1);
        assert!(matches!(
            publisher.events().as_slice(),
            [ApplicationEvent::FormCreated { form_id, details, .. }]
                if form_id == &created_form.id().to_string()
                    && details.iter().any(|detail| detail.name == "説明")
        ));
    }

    #[tokio::test]
    async fn update_form_keeps_existing_questions_when_questions_field_is_omitted() {
        let user = admin_user();
        let form_id = FormId::from(Uuid::new_v4());
        let form = sample_form(form_id);
        let repositories = FormUseCaseTestRepositories::with_active_forms(vec![form.clone()]);
        let publisher = RecordingPublisher::default();
        let usecase = FormUseCase {
            application_event_publisher: Some(&publisher),
            ..repositories.form_use_case()
        };

        let (updated_form, _) = usecase
            .update_form(
                &user,
                form.id().to_owned(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        assert_eq!(
            updated_form.questions().as_slice().len(),
            form.questions().as_slice().len()
        );
        assert_eq!(
            updated_form.questions().as_slice()[0].template_key(),
            form.questions().as_slice()[0].template_key()
        );
        assert!(publisher.events().is_empty());
    }

    #[tokio::test]
    async fn form_update_publishes_the_actual_changed_value() {
        let user = admin_user();
        let form_id = FormId::from(Uuid::new_v4());
        let repositories =
            FormUseCaseTestRepositories::with_active_forms(vec![sample_form(form_id)]);
        let publisher = RecordingPublisher::default();
        let usecase = FormUseCase {
            application_event_publisher: Some(&publisher),
            ..repositories.form_use_case()
        };

        usecase
            .update_form(
                &user,
                form_id,
                Some(FormTitle::new(
                    "Updated form".to_string().try_into().unwrap(),
                )),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        assert!(matches!(
            publisher.events().as_slice(),
            [ApplicationEvent::FormUpdated { changes, .. }]
                if changes.iter().any(|detail|
                    detail.name == "タイトル" && detail.value == "Updated form")
        ));
    }

    #[tokio::test]
    async fn form_update_does_not_publish_when_the_value_is_unchanged() {
        let user = admin_user();
        let form_id = FormId::from(Uuid::new_v4());
        let form = sample_form(form_id);
        let title = form.title().clone();
        let repositories = FormUseCaseTestRepositories::with_active_forms(vec![form]);
        let publisher = RecordingPublisher::default();
        let usecase = FormUseCase {
            application_event_publisher: Some(&publisher),
            ..repositories.form_use_case()
        };

        usecase
            .update_form(
                &user,
                form_id,
                Some(title),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        assert!(publisher.events().is_empty());
    }

    #[test]
    fn label_only_changes_are_excluded_from_form_update_notifications() {
        let form_id = FormId::from(Uuid::new_v4());
        let before = sample_form(form_id);
        let after = before
            .clone()
            .replace_label_ids(FormLabelAssignment::try_new(vec![FormLabelId::new()]).unwrap());

        assert!(form_update_details(&before, &after).is_empty());
    }

    #[tokio::test]
    async fn form_archive_and_restore_publish_events_after_success() {
        let user = admin_user();
        let form_id = FormId::from(Uuid::new_v4());
        let repositories =
            FormUseCaseTestRepositories::with_active_forms(vec![sample_form(form_id)]);
        let publisher = RecordingPublisher::default();
        let usecase = FormUseCase {
            application_event_publisher: Some(&publisher),
            ..repositories.form_use_case()
        };

        usecase.archive_form(&user, form_id).await.unwrap();
        usecase.restore_form(&user, form_id).await.unwrap();

        assert!(matches!(
            publisher.events().as_slice(),
            [
                ApplicationEvent::FormArchived { form_id: archived_id, .. },
                ApplicationEvent::FormRestored { form_id: restored_id, .. }
            ] if archived_id == &form_id.to_string() && restored_id == &form_id.to_string()
        ));
    }
}
