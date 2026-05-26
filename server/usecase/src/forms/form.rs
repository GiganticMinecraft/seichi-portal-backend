use chrono::Utc;
use domain::{
    form::{
        answer::settings::models::{AnswerVisibility, DefaultAnswerTitle, ResponsePeriod},
        models::{
            ActiveForm, ArchivedForm, FormDescription, FormId, FormLabel, FormLabelId,
            FormLabelIdSet, FormTitle, Question, QuestionSet, Visibility, WebhookUrl,
        },
    },
    repository::{
        form::{
            active_form_repository::ActiveFormRepository,
            answer_entry_set_repository::AnswerEntrySetRepository,
            archived_form_repository::ArchivedFormRepository,
            form_label_repository::FormLabelRepository,
        },
        notification_repository::NotificationRepository,
        user_repository::UserRepository,
    },
    user::models::{ActiveUser, Actor},
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
    AnswerEntrySetRepo: AnswerEntrySetRepository,
    UserRepo: UserRepository,
> {
    pub active_form_repository: &'a FormRepo,
    pub archived_form_repository: &'a ArchivedFormRepo,
    pub notification_repository: &'a NotificationRepo,
    pub form_label_repository: &'a FormLabelRepo,
    pub answer_entry_set_repository: &'a AnswerEntrySetRepo,
    pub user_repository: &'a UserRepo,
}

impl<
    R1: ActiveFormRepository,
    R2: ArchivedFormRepository,
    R3: NotificationRepository,
    R4: FormLabelRepository,
    R5: AnswerEntrySetRepository,
    R6: UserRepository,
> FormUseCase<'_, R1, R2, R3, R4, R5, R6>
{
    pub async fn create_form(
        &self,
        title: FormTitle,
        description: FormDescription,
        questions: NonEmptyVec<Question>,
        allow_temporary_answers: Option<bool>,
        user: &ActiveUser,
    ) -> Result<ActiveForm, Error> {
        use domain::form::answer::settings::models::{
            AnswerVisibility as AV, DefaultAnswerTitle as DAT, ResponsePeriod as RP,
        };
        use domain::form::answer_entry_set::models::AnswerEntrySet;

        let user_as_user = Actor::from(user.clone());

        let mut answer_entry_set =
            AnswerEntrySet::new(DAT::new(None), AV::PRIVATE, RP::try_new(None, None)?, false);
        if let Some(allow) = allow_temporary_answers {
            answer_entry_set = answer_entry_set.change_allow_temporary_answers(allow);
        }

        let answer_entry_set_guard =
            domain::types::authorization_guard::AuthorizationGuard::from(answer_entry_set.clone());
        self.answer_entry_set_repository
            .create(answer_entry_set_guard)
            .await?;

        let form = ActiveForm::new_with_answer_entry_set_id(
            title,
            description,
            QuestionSet::try_new(questions).map_err(Error::from)?,
            *answer_entry_set.id(),
        );
        let form_id = *form.id();

        self.active_form_repository
            .create(user, form.into())
            .await?;

        self.active_form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?
            .try_into_read(&user_as_user)
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
            .flat_map(|form| form.try_into_read(actor))
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
                        .map(|guard| guard.try_into_read(actor))
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
            .try_into_read(actor)?;
        let labels = self
            .form_label_repository
            .fetch_labels_by_form_id(form_id)
            .await?
            .into_iter()
            .map(|label| label.try_into_read(actor))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ActiveFormWithLabels { form, labels })
    }

    pub async fn archived_form_list(
        &self,
        actor: &ActiveUser,
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
            .flat_map(|form| form.try_into_read(&actor_user))
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
                        .try_into_read(&actor_user)?;
                    Ok::<_, Error>(ArchivedFormDetails {
                        archived_by,
                        form,
                        labels: labels
                            .into_iter()
                            .map(|label| label.try_into_read(&actor_user))
                            .collect::<Result<Vec<_>, _>>()?,
                    })
                }
            })
            .collect::<Vec<_>>();

        futures::future::try_join_all(forms_with_labels).await
    }

    pub async fn get_archived_form(
        &self,
        actor: &ActiveUser,
        form_id: FormId,
    ) -> Result<ArchivedFormDetails, Error> {
        let actor_user = Actor::from(actor.clone());
        let form = self
            .archived_form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?
            .try_into_read(&actor_user)?;
        let labels = self
            .form_label_repository
            .fetch_labels_by_form_id(form.form().id().to_owned())
            .await?
            .into_iter()
            .map(|label| label.try_into_read(&actor_user))
            .collect::<Result<Vec<_>, _>>()?;

        let archived_by = self
            .user_repository
            .find_by(form.archived_by().into_inner())
            .await?
            .ok_or(Error::from(UserNotFound))?
            .try_into_read(&actor_user)?;

        Ok(ArchivedFormDetails {
            form,
            archived_by,
            labels,
        })
    }

    pub async fn archive_form(
        &self,
        actor: &ActiveUser,
        form_id: FormId,
    ) -> Result<ArchivedForm, Error> {
        let actor_user = Actor::from(actor.clone());
        let form = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?;
        let form = form
            .try_into_read(&actor_user)?
            .archive(Utc::now(), *actor.id());
        let archived_form = self
            .archived_form_repository
            .archive(actor, form.into())
            .await?;
        archived_form.try_into_read(&actor_user).map_err(Into::into)
    }

    pub async fn restore_form(&self, actor: &ActiveUser, form_id: FormId) -> Result<(), Error> {
        let form = self
            .archived_form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?;

        self.archived_form_repository
            .restore(actor, form.into_update())
            .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_form(
        &self,
        actor: &ActiveUser,
        form_id: FormId,
        title: Option<FormTitle>,
        description: Option<FormDescription>,
        response_period: Option<ResponsePeriod>,
        webhook: Option<WebhookUrl>,
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
        let current_form_read = current_form.try_read(&actor_user)?;
        let answer_entry_set_id = *current_form_read.answer_entry_set_id();
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

            let answer_entry_set_guard = self
                .answer_entry_set_repository
                .get(answer_entry_set_id)
                .await?
                .ok_or(Error::from(FormNotFound))?;
            let answer_entry_set = answer_entry_set_guard.try_read(&actor_user)?;
            let has_answers = !answer_entry_set.entries().is_empty();
            if has_answers {
                validate_answered_form_question_update(&current_questions, questions.as_slice())?;
            }
        }

        if response_period.is_some()
            || default_answer_title.is_some()
            || answer_visibility.is_some()
            || allow_temporary_answers.is_some()
        {
            let set_guard = self
                .answer_entry_set_repository
                .get(answer_entry_set_id)
                .await?
                .ok_or(Error::from(FormNotFound))?;
            let updated_set = set_guard.into_update().map(|set| {
                let set = match answer_visibility {
                    None => set,
                    Some(v) => set.change_visibility(v),
                };
                let set = match default_answer_title {
                    None => set,
                    Some(t) => set.change_default_answer_title(t),
                };
                let set = match response_period {
                    None => set,
                    Some(p) => set.change_response_period(p),
                };
                match allow_temporary_answers {
                    None => set,
                    Some(a) => set.change_allow_temporary_answers(a),
                }
            });
            self.answer_entry_set_repository.update(updated_set).await?;
        }

        let label_ids = match label_ids {
            Some(label_ids) => {
                let label_ids = FormLabelIdSet::try_new(label_ids)?;
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
            let updated_settings = match webhook {
                None => updated_settings,
                Some(webhook) => updated_settings.change_webhook_url(webhook),
            };

            let updated_form = match title {
                None => form,
                Some(title) => form.change_title(title),
            };
            let updated_form = match description {
                None => updated_form,
                Some(description) => updated_form.change_description(description),
            };
            updated_form.change_settings(updated_settings)
        });

        let updated_form = match label_ids {
            Some(label_ids) => updated_form.map(|form| form.replace_label_ids(label_ids)),
            None => updated_form,
        };

        let updated_form = match questions {
            Some(questions) => {
                let questions = NonEmptyVec::try_new(
                    questions
                        .into_iter()
                        .map(|question| question.question)
                        .collect::<Vec<_>>(),
                )
                .map_err(Error::from)?;
                updated_form.map(|form| {
                    form.change_questions(QuestionSet::try_new(questions).expect("validated"))
                })
            }
            None => updated_form,
        };

        self.active_form_repository
            .update_form(actor, updated_form)
            .await?;

        let updated_form = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?
            .try_into_read(&actor_user)
            .map_err(|_| Error::from(FormNotFound))?;

        let label_guards = self
            .form_label_repository
            .fetch_labels_by_form_id(form_id)
            .await?;
        let labels = label_guards
            .into_iter()
            .map(|label| label.try_into_read(&actor_user))
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
        .map(|question| (question.question.id().into_inner(), &question.question))
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
    use domain::{
        form::{
            models::{
                ActiveForm, FormDescription, FormLabelIdSet, FormMeta, FormSettings, FormTitle,
            },
            question::models::{QuestionId, QuestionSet, QuestionType},
        },
        repository::{
            form::{
                active_form_repository::MockActiveFormRepository,
                answer_entry_set_repository::MockAnswerEntrySetRepository,
                archived_form_repository::MockArchivedFormRepository,
                form_label_repository::MockFormLabelRepository,
            },
            notification_repository::MockNotificationRepository,
            user_repository::MockUserRepository,
        },
        user::models::{ActiveUser, Role},
    };
    use types::non_empty_vec::NonEmptyVec;
    use uuid::Uuid;

    fn admin_user() -> ActiveUser {
        ActiveUser::new("admin".to_string(), Uuid::nil().into(), Role::Administrator)
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
        ActiveForm::from_raw_parts(
            form_id,
            FormTitle::new("Form".to_string().try_into().unwrap()),
            FormDescription::new("description".to_string()),
            FormMeta::new(),
            FormSettings::new(),
            questions,
            FormLabelIdSet::empty(),
            domain::form::answer_entry_set::models::AnswerEntrySetId::new(),
        )
    }

    fn text_question(question_id: QuestionId, position: u16, template_key: &str) -> Question {
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

        let mut active_form_repository = MockActiveFormRepository::new();
        active_form_repository
            .expect_create()
            .times(1)
            .returning(|_, _| Ok(()));
        active_form_repository
            .expect_get()
            .times(1)
            .returning(move |form_id| Ok(Some(sample_form(form_id).into())));

        let form_label_repository = MockFormLabelRepository::new();
        let mut answer_entry_set_repository = MockAnswerEntrySetRepository::new();
        answer_entry_set_repository
            .expect_create()
            .times(1)
            .returning(|_| Ok(()));
        let archived_form_repository = MockArchivedFormRepository::new();
        let notification_repository = MockNotificationRepository::new();
        let user_repository = MockUserRepository::new();

        let usecase = FormUseCase {
            active_form_repository: &active_form_repository,
            archived_form_repository: &archived_form_repository,
            notification_repository: &notification_repository,
            form_label_repository: &form_label_repository,
            answer_entry_set_repository: &answer_entry_set_repository,
            user_repository: &user_repository,
        };

        let created_form = usecase
            .create_form(
                FormTitle::new("Form".to_string().try_into().unwrap()),
                FormDescription::new("description".to_string()),
                input_questions,
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
        let mut active_form_repository = MockActiveFormRepository::new();
        active_form_repository
            .expect_get()
            .times(3)
            .returning(move |_| Ok(Some(sample_form(form_id).into())));
        active_form_repository
            .expect_update_form()
            .times(1)
            .returning(|_, _| Ok(()));

        let mut form_label_repository = MockFormLabelRepository::new();
        form_label_repository
            .expect_fetch_labels_by_form_id()
            .times(1)
            .returning(|_| Ok(vec![]));

        let answer_entry_set_repository = MockAnswerEntrySetRepository::new();
        let archived_form_repository = MockArchivedFormRepository::new();
        let notification_repository = MockNotificationRepository::new();
        let user_repository = MockUserRepository::new();

        let usecase = FormUseCase {
            active_form_repository: &active_form_repository,
            archived_form_repository: &archived_form_repository,
            notification_repository: &notification_repository,
            form_label_repository: &form_label_repository,
            answer_entry_set_repository: &answer_entry_set_repository,
            user_repository: &user_repository,
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
