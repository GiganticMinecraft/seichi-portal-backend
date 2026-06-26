use chrono::Utc;
use domain::{
    account::models::AccountUser,
    auth::Actor,
    form::answer::TemporaryAnswerAuthor,
    form::{
        answer::{
            AnswerAuthor, AnswerEntry, AnswerId, AnswerLabel, AnswerSubmitter, AnswerTitle,
            FormAnswerContent, PostedAnswerContents,
        },
        models::{ActiveForm, FormId},
        service::DefaultAnswerTitleDomainService,
    },
    repository::user_repository::UserRepository,
    repository::{
        answer_submitter_restriction_repository::AnswerSubmitterRestrictionRepository,
        form::{
            active_form_repository::ActiveFormRepository,
            answer_entry_repository::AnswerEntryRepository,
            answer_label_repository::AnswerLabelRepository,
        },
    },
    types::authorization_guard::{Allowed, Read},
};
use errors::{
    Error,
    domain::DomainError,
    usecase::UseCaseError::{AnswerNotFound, FormNotFound},
};
use futures::{StreamExt, stream};

use crate::{
    forms::discord_answer_webhook::{
        DiscordAnswerWebhookField, DiscordAnswerWebhookNotification, DiscordAnswerWebhookNotifier,
    },
    models::AnswerDetails,
    user_reference_resolver::resolve_user_references,
};
use common::config::FRONTEND;

pub struct AnswerUseCase<
    'a,
    FormRepo: ActiveFormRepository,
    AnswerLabelRepo: AnswerLabelRepository,
    UserRepo: UserRepository,
    AnswerSubmitterRestrictionRepo: AnswerSubmitterRestrictionRepository,
    AnswerEntryRepo: AnswerEntryRepository,
> {
    pub active_form_repository: &'a FormRepo,
    pub answer_label_repository: &'a AnswerLabelRepo,
    pub user_repository: &'a UserRepo,
    pub answer_submitter_restriction_repository: &'a AnswerSubmitterRestrictionRepo,
    pub answer_entry_repository: &'a AnswerEntryRepo,
    pub discord_answer_webhook_notifier: Option<&'a dyn DiscordAnswerWebhookNotifier>,
}

impl<
    R1: ActiveFormRepository,
    R2: AnswerLabelRepository,
    R3: UserRepository,
    R4: AnswerSubmitterRestrictionRepository,
    R5: AnswerEntryRepository,
> AnswerUseCase<'_, R1, R2, R3, R4, R5>
{
    async fn read_form(
        &self,
        form_id: FormId,
        actor: &Actor,
    ) -> Result<Allowed<ActiveForm, Read>, Error> {
        self.active_form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?
            .try_read(actor.clone())
            .map_err(Into::into)
    }

    async fn build_answer_details(
        &self,
        actor: &AccountUser,
        form_id: FormId,
        form_answer: Allowed<AnswerEntry, Read>,
        labels: Vec<AnswerLabel>,
    ) -> Result<AnswerDetails, Error> {
        let user_ids = form_answer
            .author()
            .authenticated_user_id()
            .into_iter()
            .collect();

        let users = resolve_user_references(self.user_repository, actor, user_ids).await?;

        let author = match form_answer.author() {
            AnswerAuthor::AuthenticatedUser(user_id) => Actor::AccountUser(
                users
                    .get(user_id)
                    .cloned()
                    .ok_or(Error::from(errors::usecase::UseCaseError::UserNotFound))?,
            ),
            AnswerAuthor::Temporary(temporary_user) => {
                Actor::TemporaryAnswerAuthor(temporary_user.clone())
            }
        };

        Ok(AnswerDetails {
            form_id,
            form_answer: form_answer.into_inner(),
            author,
            labels,
        })
    }

    async fn notify_discord_answer_webhook(
        &self,
        form: &Allowed<ActiveForm, Read>,
        answer_entry: &Allowed<AnswerEntry, domain::types::authorization_guard::Create>,
        respondent: String,
    ) {
        let Some(notifier) = self.discord_answer_webhook_notifier else {
            return;
        };
        let Some(discord_webhook_url) = form
            .settings()
            .discord_webhook_url(&Actor::System)
            .ok()
            .cloned()
            .and_then(|url| url.into_inner())
            .map(|url| url.into_inner())
        else {
            return;
        };

        let form_id = form.id().into_inner().to_string();
        let answer_id = answer_entry.id().into_inner().to_string();
        let answer_url = format!("{}/forms/{form_id}/answers/{answer_id}", FRONTEND.url);
        let answer_title = answer_entry
            .title()
            .to_owned()
            .into_inner()
            .map(|title| title.into_inner())
            .unwrap_or_default();
        let questions = form.questions().as_slice();
        let answer_fields = answer_entry
            .contents()
            .iter()
            .map(|content| {
                let question_title = questions
                    .iter()
                    .find(|question| question.id() == content.question_id)
                    .map(|question| question.title().to_owned().into_inner())
                    .unwrap_or_else(|| "不明な質問".to_string());

                DiscordAnswerWebhookField::new(question_title, content.answer.clone())
            })
            .collect::<Vec<_>>();
        let fields = [
            vec![
                DiscordAnswerWebhookField::new(
                    "フォーム名".to_string(),
                    form.title().to_owned().into_inner().into_inner(),
                ),
                DiscordAnswerWebhookField::new("タイトル".to_string(), answer_title),
                DiscordAnswerWebhookField::new("回答者".to_string(), respondent),
            ],
            answer_fields,
        ]
        .into_iter()
        .flatten()
        .collect();

        notifier
            .notify_answer_posted(DiscordAnswerWebhookNotification {
                discord_webhook_url,
                answer_url,
                form_id,
                answer_id,
                fields,
            })
            .await;
    }

    pub async fn post_answers(
        &self,
        user: AccountUser,
        form_id: FormId,
        answers: Vec<FormAnswerContent>,
    ) -> Result<(), Error> {
        let actor = Actor::from(user.clone());

        let form_guard = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?;
        let form = form_guard.try_read(actor.clone())?;
        let questions = form.value().questions().as_slice().to_vec();
        let posted_answers = PostedAnswerContents::try_new(&questions, answers)?;
        let restriction = self
            .answer_submitter_restriction_repository
            .fetch_active_by_submitter_id(user.id().into_inner())
            .await?
            .map(|restriction| {
                restriction
                    .try_read(actor.clone())
                    .map(|restriction| restriction.into_inner())
            })
            .transpose()?;
        let submitter = AnswerSubmitter::try_new(user.clone(), restriction, Utc::now())?;

        let title = DefaultAnswerTitleDomainService::to_answer_title_from_questions(
            form.value()
                .answer_settings()
                .default_answer_title()
                .to_owned(),
            &questions,
            &posted_answers,
            user.name(),
        )?;

        let answer_entry = form.try_accept_answer(submitter, title, posted_answers)?;

        self.answer_entry_repository
            .post(&form, &answer_entry)
            .await?;

        self.notify_discord_answer_webhook(&form, &answer_entry, user.name().to_string())
            .await;

        Ok(())
    }

    pub async fn post_temporary_answers(
        &self,
        temporary_user: TemporaryAnswerAuthor,
        form_id: FormId,
        answers: Vec<FormAnswerContent>,
    ) -> Result<(), Error> {
        let actor = Actor::from(temporary_user.clone());

        let form_guard = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?;
        let form = form_guard.try_read(actor.clone())?;
        let questions = form.value().questions().as_slice().to_vec();
        let posted_answers = PostedAnswerContents::try_new(&questions, answers)?;

        let title = DefaultAnswerTitleDomainService::to_answer_title_from_questions(
            form.value()
                .answer_settings()
                .default_answer_title()
                .to_owned(),
            &questions,
            &posted_answers,
            temporary_user.name(),
        )?;

        let respondent = format!(
            "{} ({})",
            temporary_user.name(),
            temporary_user.contact_text()
        );
        let answer_entry =
            form.try_accept_temporary_answer(temporary_user, title, posted_answers)?;

        self.answer_entry_repository
            .post(&form, &answer_entry)
            .await?;

        self.notify_discord_answer_webhook(&form, &answer_entry, respondent)
            .await;

        Ok(())
    }

    pub async fn get_answers(
        &self,
        form_id: FormId,
        answer_id: AnswerId,
        user: &AccountUser,
    ) -> Result<AnswerDetails, Error> {
        let actor = Actor::from(user.clone());
        let form = self.read_form(form_id, &actor).await?;

        let form_answer = self
            .answer_entry_repository
            .get(&form, answer_id)
            .await?
            .ok_or(AnswerNotFound)?;

        let labels = self
            .answer_label_repository
            .get_labels_for_answers_by_answer_id(answer_id)
            .await?
            .into_iter()
            .map(|label| {
                label
                    .try_read(actor.clone())
                    .map(|label| label.into_inner())
            })
            .collect::<Result<Vec<_>, _>>()?;

        self.build_answer_details(user, form_id, form_answer, labels)
            .await
    }

    pub async fn get_answers_by_form_id(
        &self,
        form_id: FormId,
        actor: &AccountUser,
    ) -> Result<Vec<AnswerDetails>, Error> {
        let actor_ref = Actor::from(actor.clone());
        let form = self.read_form(form_id, &actor_ref).await?;

        let visible_answers = self.answer_entry_repository.list_by_form(&form).await?;

        stream::iter(visible_answers)
            .then(|form_answer| async {
                let answer_id = *form_answer.id();
                let labels = self
                    .answer_label_repository
                    .get_labels_for_answers_by_answer_id(answer_id)
                    .await?
                    .into_iter()
                    .map(|label| {
                        label
                            .try_read(actor_ref.clone())
                            .map(|label| label.into_inner())
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                self.build_answer_details(actor, form_id, form_answer, labels)
                    .await
            })
            .collect::<Vec<Result<AnswerDetails, Error>>>()
            .await
            .into_iter()
            .collect()
    }

    pub async fn get_all_answers(&self, user: &AccountUser) -> Result<Vec<AnswerDetails>, Error> {
        let actor_ref = Actor::from(user.clone());
        let readable_forms = self
            .active_form_repository
            .list(None, None)
            .await?
            .into_iter()
            .filter_map(|form| form.try_read(actor_ref.clone()).ok())
            .collect::<Vec<_>>();

        let visible_answers: Vec<(FormId, Allowed<AnswerEntry, Read>)> = self
            .answer_entry_repository
            .list_all(&readable_forms)
            .await?
            .into_iter()
            .map(|entry| (*entry.value().form_id(), entry))
            .collect();

        stream::iter(visible_answers)
            .then(|(form_id, form_answer)| {
                let user = user.clone();
                async move {
                    let actor_ref = Actor::from(user.clone());

                    let answer_id = *form_answer.id();
                    let labels = self
                        .answer_label_repository
                        .get_labels_for_answers_by_answer_id(answer_id)
                        .await?
                        .into_iter()
                        .map(|label| {
                            label
                                .try_read(actor_ref.clone())
                                .map(|label| label.into_inner())
                        })
                        .collect::<Result<Vec<_>, _>>()?;

                    self.build_answer_details(&user, form_id, form_answer, labels)
                        .await
                }
            })
            .collect::<Vec<Result<AnswerDetails, Error>>>()
            .await
            .into_iter()
            .filter(|result| {
                !matches!(
                    result,
                    Err(Error::Domain {
                        source: DomainError::Forbidden
                    })
                )
            })
            .collect()
    }

    pub async fn update_answer_meta(
        &self,
        form_id: FormId,
        answer_id: AnswerId,
        actor: &AccountUser,
        title: Option<AnswerTitle>,
    ) -> Result<AnswerDetails, Error> {
        let actor_ref = Actor::from(actor.clone());
        let form = self.read_form(form_id, &actor_ref).await?;

        let form_answer = match title {
            Some(title) => {
                let form_update = self
                    .active_form_repository
                    .get(form_id)
                    .await?
                    .ok_or(FormNotFound)?
                    .into_update()
                    .try_update(actor_ref.clone())?;
                let entry = self
                    .answer_entry_repository
                    .get(&form, answer_id)
                    .await?
                    .ok_or(AnswerNotFound)?;
                let updated_entry = form_update.change_entry_title(entry.into_inner(), title)?;

                self.answer_entry_repository
                    .update(&form_update, &updated_entry)
                    .await?;

                self.answer_entry_repository
                    .get(&form, answer_id)
                    .await?
                    .ok_or(AnswerNotFound)?
            }
            None => self
                .answer_entry_repository
                .get(&form, answer_id)
                .await?
                .ok_or(AnswerNotFound)?,
        };

        let labels = self
            .answer_label_repository
            .get_labels_for_answers_by_answer_id(answer_id)
            .await?
            .into_iter()
            .map(|label| {
                label
                    .try_read(actor_ref.clone())
                    .map(|label| label.into_inner())
            })
            .collect::<Result<Vec<_>, _>>()?;

        self.build_answer_details(actor, form_id, form_answer, labels)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use domain::{
        account::models::Role,
        form::{
            answer::{
                AnswerLabelId, AnswerSubmitterRestriction, AnswerSubmitterRestrictionReason,
                FormAnswerContentId,
            },
            models::{FormDescription, FormTitle, QuestionSet},
            question::Question,
        },
        repository::form::answer_label_repository::AnswerLabelRepository,
        types::authorization_guard::{AuthorizationGuard, Create, Delete, Update},
    };
    use errors::domain::DomainError;
    use types::non_empty_vec::NonEmptyVec;
    use uuid::Uuid;

    use crate::test_utils::repositories::FormUseCaseTestRepositories;

    #[derive(Default)]
    struct EmptyAnswerLabelRepository;

    #[async_trait]
    impl AnswerLabelRepository for EmptyAnswerLabelRepository {
        async fn create_label_for_answers(
            &self,
            _label: Allowed<AnswerLabel, Create>,
        ) -> Result<(), Error> {
            Ok(())
        }

        async fn get_labels_for_answers(
            &self,
        ) -> Result<Vec<AuthorizationGuard<AnswerLabel, Read>>, Error> {
            Ok(vec![])
        }

        async fn get_label_for_answers(
            &self,
            _label_id: AnswerLabelId,
        ) -> Result<Option<AuthorizationGuard<AnswerLabel, Read>>, Error> {
            Ok(None)
        }

        async fn get_labels_for_answers_by_label_ids(
            &self,
            _label_ids: Vec<AnswerLabelId>,
        ) -> Result<Vec<AuthorizationGuard<AnswerLabel, Read>>, Error> {
            Ok(vec![])
        }

        async fn get_labels_for_answers_by_answer_id(
            &self,
            _answer_id: AnswerId,
        ) -> Result<Vec<AuthorizationGuard<AnswerLabel, Read>>, Error> {
            Ok(vec![])
        }

        async fn delete_label_for_answers(
            &self,
            _label: Allowed<AnswerLabel, Delete>,
        ) -> Result<(), Error> {
            Ok(())
        }

        async fn edit_label_for_answers(
            &self,
            _label: Allowed<AnswerLabel, Update>,
        ) -> Result<(), Error> {
            Ok(())
        }

        async fn replace_answer_labels(
            &self,
            _answer_id: AnswerId,
            _labels: Vec<Allowed<AnswerLabel, Update>>,
        ) -> Result<(), Error> {
            Ok(())
        }

        async fn size(&self) -> Result<u32, Error> {
            Ok(0)
        }
    }

    fn active_user(name: &str, role: Role) -> AccountUser {
        AccountUser::new(name.to_string(), Uuid::new_v4().into(), role)
    }

    fn sample_form() -> ActiveForm {
        let question = Question::new_text(
            "body".to_string().try_into().unwrap(),
            0,
            "Body".to_string().try_into().unwrap(),
            None,
            true,
        )
        .unwrap();

        ActiveForm::new(
            FormTitle::new("Form".to_string().try_into().unwrap()),
            FormDescription::new("description".to_string()),
            QuestionSet::try_new(NonEmptyVec::try_new(vec![question]).unwrap()).unwrap(),
        )
    }

    #[tokio::test]
    async fn post_answers_rejects_user_with_active_answer_submitter_restriction() {
        let form = sample_form();
        let user = active_user("user", Role::StandardUser);
        let now = Utc::now();
        let restriction = AnswerSubmitterRestriction::new(
            *user.id(),
            AnswerSubmitterRestrictionReason::new("spam".to_string().try_into().unwrap()),
            Uuid::new_v4().into(),
            now,
            None,
        )
        .unwrap();
        let answer = FormAnswerContent {
            id: FormAnswerContentId::new(),
            question_id: (*form.questions().as_slice()[0].id()).into(),
            answer: "answer".to_string(),
        };

        let repositories = FormUseCaseTestRepositories::with_active_forms(vec![form.clone()]);
        repositories
            .answer_submitter_restriction_repository
            .save_answer_submitter_restriction(restriction);
        let empty_answer_label_repository = EmptyAnswerLabelRepository;
        let usecase = AnswerUseCase {
            active_form_repository: &repositories.active_form_repository,
            answer_label_repository: &empty_answer_label_repository,
            user_repository: &repositories.user_repository,
            answer_submitter_restriction_repository: &repositories
                .answer_submitter_restriction_repository,
            answer_entry_repository: &repositories.answer_entry_repository,
            discord_answer_webhook_notifier: None,
        };

        let result = usecase.post_answers(user, *form.id(), vec![answer]).await;

        assert_eq!(
            result,
            Err(DomainError::AnswerSubmissionRestricted {
                reason: "spam".to_string(),
                expires_at: None,
            }
            .into())
        );
        assert_eq!(
            repositories.answer_entry_repository.size().await.unwrap(),
            0
        );
    }
}
