use chrono::Utc;
use domain::{
    account::models::AccountUser,
    auth::Actor,
    form::models::{
        ActiveForm, AnswerAcceptancePeriod, AnswerSettings, AnswerVisibility, ArchivedForm,
        DefaultAnswerTitle, DiscordWebhookUrl, FormDescription, FormId, FormLabel,
        FormLabelAssignment, FormLabelId, FormSettings, FormTitle, Question, QuestionSet,
        Visibility,
    },
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
    usecase::UseCaseError::{FormNotFound, LabelNotFound, UserNotFound},
};
use std::collections::{BTreeSet, HashMap};
use types::non_empty_vec::NonEmptyVec;

use crate::models::{ActiveFormWithLabels, ArchivedFormDetails, UpsertQuestionInput};

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
    #[allow(clippy::too_many_arguments)]
    pub async fn create_form(
        &self,
        title: FormTitle,
        description: FormDescription,
        questions: NonEmptyVec<Question>,
        discord_webhook_url: Option<DiscordWebhookUrl>,
        visibility: Option<Visibility>,
        allow_temporary_answers: Option<bool>,
        answer_visibility: Option<AnswerVisibility>,
        acceptance_period: Option<AnswerAcceptancePeriod>,
        default_answer_title: Option<DefaultAnswerTitle>,
        user: &AccountUser,
    ) -> Result<ActiveForm, Error> {
        let user_as_user = Actor::from(user.clone());

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

        let answer_settings = AnswerSettings::default();
        let answer_settings = match allow_temporary_answers {
            Some(allow) => answer_settings.change_allow_temporary_answers(allow),
            None => answer_settings,
        };
        let answer_settings = match answer_visibility {
            Some(visibility) => answer_settings.change_visibility(visibility),
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

        self.active_form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?
            .try_read(user_as_user.clone())
            .map(|form| form.into_inner())
            .map_err(Error::from)
    }

    /// `actor` が参照可能なフォームのリストを取得する
    pub async fn form_list(
        &self,
        actor: &Actor,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> Result<Vec<(ActiveForm, Vec<FormLabel>)>, Error> {
        let forms = self
            .active_form_repository
            .list(offset, limit)
            .await?
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

        Ok(forms_with_labels)
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
        offset: Option<u32>,
        limit: Option<u32>,
        query: Option<String>,
    ) -> Result<Vec<ArchivedFormDetails>, Error> {
        let actor_user = Actor::from(actor.clone());
        let forms = self
            .archived_form_repository
            .list(offset, limit, query)
            .await?
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

        futures::future::try_join_all(forms_with_labels).await
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
        archived_form
            .try_read(actor_user.clone())
            .map(|form| form.into_inner())
            .map_err(Into::into)
    }

    pub async fn restore_form(&self, actor: &AccountUser, form_id: FormId) -> Result<(), Error> {
        let form = self
            .archived_form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?;

        self.archived_form_repository
            .restore(form.into_update().try_update(Actor::from(actor.clone()))?)
            .await
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
        allow_temporary_answers: Option<bool>,
        answer_visibility: Option<AnswerVisibility>,
        questions: Option<Vec<UpsertQuestionInput>>,
        label_ids: Option<Vec<FormLabelId>>,
    ) -> Result<(ActiveForm, Vec<FormLabel>), Error> {
        let actor_user = Actor::from(actor.clone());
        let current_form = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?;
        let current_form_read = current_form.try_read(actor_user.clone())?;
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
                .list_by_form(&current_form_read)
                .await?
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

        Ok((updated_form, labels))
    }
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
    use types::non_empty_vec::NonEmptyVec;
    use uuid::Uuid;

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
        let usecase = repositories.form_use_case();

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
                &user,
            )
            .await
            .unwrap();

        assert_eq!(created_form.questions().as_slice().len(), 1);
    }

    #[tokio::test]
    async fn update_form_keeps_existing_questions_when_questions_field_is_omitted() {
        let user = admin_user();
        let form_id = FormId::from(Uuid::new_v4());
        let form = sample_form(form_id);
        let repositories = FormUseCaseTestRepositories::with_active_forms(vec![form.clone()]);
        let usecase = repositories.form_use_case();

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
    }
}
