#[cfg(test)]
use chrono::Utc;
use domain::{
    account::models::AccountUser,
    auth::Actor,
    form::answer::TemporaryAnswerAuthor,
    form::{
        answer::{
            AnswerAuthor, AnswerAuthorDisclosure, AnswerEntry, AnswerId, AnswerLabel,
            AnswerPagePosition, AnswerPublication, AnswerRelation, AnswerTitle, FormAnswerContent,
            PostedAnswerContents,
        },
        models::{ActiveForm, FormId},
        service::DefaultAnswerTitleDomainService,
    },
    pagination::{Page, PageRequest},
    repository::user_repository::UserRepository,
    repository::{
        form::{
            active_form_repository::ActiveFormRepository,
            answer_entry_repository::AnswerEntryRepository,
            answer_label_repository::AnswerLabelRepository,
            answer_relation_repository::AnswerRelationRepository,
        },
        form_submission_restriction_repository::FormSubmissionRestrictionRepository,
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
    application_event::{
        AnswerSubmissionActor, ApplicationActor, ApplicationEvent, ApplicationEventPublisher,
        EventDetail,
    },
    forms::discord_answer_webhook::{
        DiscordAnswerWebhookField, DiscordAnswerWebhookNotification, DiscordAnswerWebhookNotifier,
    },
    models::{AnswerDetails, PublishedAnswerAuthor, PublishedAnswerEntry},
    user_reference_resolver::resolve_user_references,
};
use common::config::FRONTEND;

pub struct AnswerUseCase<
    'a,
    FormRepo: ActiveFormRepository,
    AnswerLabelRepo: AnswerLabelRepository,
    UserRepo: UserRepository,
    FormSubmissionRestrictionRepo: FormSubmissionRestrictionRepository,
    AnswerEntryRepo: AnswerEntryRepository,
    AnswerRelationRepo: AnswerRelationRepository,
> {
    pub active_form_repository: &'a FormRepo,
    pub answer_label_repository: &'a AnswerLabelRepo,
    pub user_repository: &'a UserRepo,
    pub form_submission_restriction_repository: &'a FormSubmissionRestrictionRepo,
    pub answer_entry_repository: &'a AnswerEntryRepo,
    pub answer_relation_repository: &'a AnswerRelationRepo,
    pub discord_answer_webhook_notifier: Option<&'a dyn DiscordAnswerWebhookNotifier>,
    pub application_event_publisher: Option<&'a dyn ApplicationEventPublisher>,
}

impl<
    R1: ActiveFormRepository,
    R2: AnswerLabelRepository,
    R3: UserRepository,
    R4: FormSubmissionRestrictionRepository,
    R5: AnswerEntryRepository,
    R6: AnswerRelationRepository,
> AnswerUseCase<'_, R1, R2, R3, R4, R5, R6>
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
        author_disclosure: AnswerAuthorDisclosure,
        labels: Vec<AnswerLabel>,
    ) -> Result<AnswerDetails, Error> {
        let author = match author_disclosure {
            AnswerAuthorDisclosure::Anonymous => PublishedAnswerAuthor::Anonymous,
            AnswerAuthorDisclosure::Disclosed => {
                let user_ids = form_answer
                    .author()
                    .authenticated_user_id()
                    .into_iter()
                    .collect();
                let users = resolve_user_references(self.user_repository, actor, user_ids).await?;

                match form_answer.author() {
                    AnswerAuthor::AuthenticatedUser(user_id) => {
                        PublishedAnswerAuthor::AuthenticatedUser(
                            users
                                .get(user_id)
                                .cloned()
                                .ok_or(Error::from(errors::usecase::UseCaseError::UserNotFound))?,
                        )
                    }
                    AnswerAuthor::Temporary(temporary_user) => {
                        PublishedAnswerAuthor::Temporary(temporary_user.clone())
                    }
                }
            }
        };

        let related_answers = self
            .visible_related_answers(actor, *form_answer.id())
            .await?;

        Ok(AnswerDetails {
            form_id,
            answer: PublishedAnswerEntry::new(form_answer.into_inner(), author),
            labels,
            related_answers,
        })
    }

    async fn visible_related_answers(
        &self,
        user: &AccountUser,
        answer_id: AnswerId,
    ) -> Result<
        Vec<domain::repository::form::answer_relation_repository::RelatedAnswerReference>,
        Error,
    > {
        use domain::{
            account::models::Role,
            repository::form::answer_relation_repository::RelatedAnswerLifecycle,
        };

        let actor = Actor::from(user.clone());
        let is_administrator = user.role() == &Role::Administrator;
        let related_answers = self
            .answer_relation_repository
            .list_for_answer(answer_id)
            .await?;

        stream::iter(related_answers)
            .then(|related_answer| {
                let actor = actor.clone();
                async move {
                    if is_administrator {
                        return Ok(Some(related_answer));
                    }
                    if related_answer.lifecycle == RelatedAnswerLifecycle::Archived {
                        return Ok(None);
                    }
                    let Some(form) = self
                        .active_form_repository
                        .get(related_answer.form_id)
                        .await?
                    else {
                        return Ok(None);
                    };
                    let Ok(form) = form.try_read(actor.clone()) else {
                        return Ok(None);
                    };
                    match self
                        .answer_entry_repository
                        .get(&form, related_answer.answer_id)
                        .await
                    {
                        Ok(answer) => Ok(answer.map(|_| related_answer)),
                        Err(Error::Domain {
                            source: DomainError::Forbidden,
                        }) => Ok(None),
                        Err(error) => Err(error),
                    }
                }
            })
            .collect::<Vec<Result<Option<_>, Error>>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map(|answers| answers.into_iter().flatten().collect())
    }

    async fn notify_discord_answer_webhook(
        &self,
        form: &Allowed<ActiveForm, Read>,
        answer_entry: &Allowed<AnswerEntry, domain::types::authorization_guard::Create>,
        author_disclosure: AnswerAuthorDisclosure,
        respondent: &str,
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
        let title = answer_entry
            .title()
            .to_owned()
            .into_inner()
            .map(|title| title.into_inner())
            .unwrap_or_else(|| "回答が送信されました".to_string());
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
                DiscordAnswerWebhookField::new(
                    "回答者".to_string(),
                    match author_disclosure {
                        AnswerAuthorDisclosure::Disclosed => respondent.to_owned(),
                        AnswerAuthorDisclosure::Anonymous => "回答者は非公開です".to_string(),
                    },
                ),
            ],
            answer_fields,
        ]
        .into_iter()
        .flatten()
        .collect();

        notifier
            .notify_answer_posted(DiscordAnswerWebhookNotification {
                discord_webhook_url,
                title,
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
        let submitter = super::submission::authorize_form_submission(
            user.clone(),
            self.form_submission_restriction_repository,
        )
        .await?;

        let title = DefaultAnswerTitleDomainService::to_answer_title_from_questions(
            form.value()
                .answer_settings()
                .default_answer_title()
                .to_owned(),
            form.value().title(),
            &questions,
            &posted_answers,
            form.answer_settings()
                .author_publication_policy()
                .default_title_author_name(user.name()),
        )?;

        let answer_entry = form.try_accept_answer(submitter, title, posted_answers)?;

        self.answer_entry_repository
            .post(&form, &answer_entry)
            .await?;

        if let Some(publisher) = self.application_event_publisher {
            publisher.publish(answer_submitted_event(
                match form
                    .answer_settings()
                    .author_disclosure_for(&Actor::Anonymous)
                {
                    AnswerAuthorDisclosure::Disclosed => {
                        AnswerSubmissionActor::Identified(ApplicationActor::from(&user))
                    }
                    AnswerAuthorDisclosure::Anonymous => AnswerSubmissionActor::AuthorHidden,
                },
                &form,
                &answer_entry,
            ));
        }

        self.notify_discord_answer_webhook(
            &form,
            &answer_entry,
            form.answer_settings()
                .author_disclosure_for(&Actor::Anonymous),
            user.name(),
        )
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
        let application_actor = ApplicationActor::from(&temporary_user);

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
            form.value().title(),
            &questions,
            &posted_answers,
            form.answer_settings()
                .author_publication_policy()
                .default_title_author_name(temporary_user.name()),
        )?;

        let respondent = temporary_user.name().to_owned();
        let answer_entry =
            form.try_accept_temporary_answer(temporary_user, title, posted_answers)?;

        self.answer_entry_repository
            .post(&form, &answer_entry)
            .await?;

        if let Some(publisher) = self.application_event_publisher {
            publisher.publish(answer_submitted_event(
                match form
                    .answer_settings()
                    .author_disclosure_for(&Actor::Anonymous)
                {
                    AnswerAuthorDisclosure::Disclosed => {
                        AnswerSubmissionActor::Identified(application_actor)
                    }
                    AnswerAuthorDisclosure::Anonymous => AnswerSubmissionActor::AuthorHidden,
                },
                &form,
                &answer_entry,
            ));
        }

        self.notify_discord_answer_webhook(
            &form,
            &answer_entry,
            form.answer_settings()
                .author_disclosure_for(&Actor::Anonymous),
            &respondent,
        )
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

        let author_disclosure = form.answer_settings().author_disclosure_for(&actor);
        self.build_answer_details(user, form_id, form_answer, author_disclosure, labels)
            .await
    }

    pub async fn get_answers_by_form_id(
        &self,
        form_id: FormId,
        actor: &AccountUser,
        request: PageRequest<AnswerPagePosition>,
    ) -> Result<Page<AnswerDetails, AnswerPagePosition>, Error> {
        let actor_ref = Actor::from(actor.clone());
        let form = self.read_form(form_id, &actor_ref).await?;

        let page = self
            .answer_entry_repository
            .list_by_form(&form, request)
            .await?;
        let (visible_answers, next) = page.into_parts();
        let author_disclosure = form.answer_settings().author_disclosure_for(&actor_ref);

        let answers = stream::iter(visible_answers)
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

                self.build_answer_details(actor, form_id, form_answer, author_disclosure, labels)
                    .await
            })
            .collect::<Vec<Result<AnswerDetails, Error>>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Page::new(answers, next))
    }

    async fn readable_forms(&self, actor: &Actor) -> Result<Vec<Allowed<ActiveForm, Read>>, Error> {
        Ok(self
            .active_form_repository
            .list_all()
            .await?
            .into_iter()
            .filter_map(|form| form.try_read(actor.clone()).ok())
            .collect())
    }

    pub async fn get_all_answers(
        &self,
        user: &AccountUser,
        request: PageRequest<AnswerPagePosition>,
    ) -> Result<Page<AnswerDetails, AnswerPagePosition>, Error> {
        let actor_ref = Actor::from(user.clone());
        let readable_forms = self.readable_forms(&actor_ref).await?;

        let page = self
            .answer_entry_repository
            .list_all(&readable_forms, request)
            .await?;
        let (visible_answers, next) = page.into_parts();
        let publication_by_form_id = readable_forms
            .iter()
            .map(|form| {
                (
                    *form.id(),
                    form.answer_settings().author_disclosure_for(&actor_ref),
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        let visible_answers: Vec<(FormId, AnswerAuthorDisclosure, Allowed<AnswerEntry, Read>)> =
            visible_answers
                .into_iter()
                .filter_map(|entry| {
                    let form_id = *entry.value().form_id();
                    publication_by_form_id
                        .get(&form_id)
                        .copied()
                        .map(|disclosure| (form_id, disclosure, entry))
                })
                .collect();

        let answers = stream::iter(visible_answers)
            .then(|(form_id, author_disclosure, form_answer)| {
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

                    self.build_answer_details(
                        &user,
                        form_id,
                        form_answer,
                        author_disclosure,
                        labels,
                    )
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
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Page::new(answers, next))
    }

    pub async fn update_answer_meta(
        &self,
        form_id: FormId,
        answer_id: AnswerId,
        actor: &AccountUser,
        title: Option<AnswerTitle>,
        publication: Option<AnswerPublication>,
        related_answer_ids: Option<Vec<AnswerId>>,
    ) -> Result<AnswerDetails, Error> {
        let actor_ref = Actor::from(actor.clone());
        let form = self.read_form(form_id, &actor_ref).await?;
        let related_answer_ids = related_answer_ids
            .map(|related_answer_ids| validate_related_answer_ids(answer_id, related_answer_ids))
            .transpose()?;

        if let Some(related_answer_ids) = &related_answer_ids {
            let form_update = self
                .active_form_repository
                .get(form_id)
                .await?
                .ok_or(FormNotFound)?
                .into_update()
                .try_update(actor_ref.clone())?;
            let answer = self
                .answer_entry_repository
                .get(&form, answer_id)
                .await?
                .ok_or(AnswerNotFound)?;
            let answer_update = form_update.authorize_entry_update(answer.into_inner())?;
            self.answer_relation_repository
                .validate_replace_for_answer(&answer_update, related_answer_ids)
                .await?;
        }

        let form_answer = match related_answer_ids {
            Some(related_answer_ids) => {
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
                let updated_entry =
                    form_update.change_entry_meta(entry.into_inner(), title, publication)?;

                self.answer_relation_repository
                    .update_answer_meta_and_replace(updated_entry, related_answer_ids)
                    .await?;

                self.answer_entry_repository
                    .get(&form, answer_id)
                    .await?
                    .ok_or(AnswerNotFound)?
            }
            None => match (title, publication) {
                (None, None) => self
                    .answer_entry_repository
                    .get(&form, answer_id)
                    .await?
                    .ok_or(AnswerNotFound)?,
                (title, publication) => {
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
                    let updated_entry =
                        form_update.change_entry_meta(entry.into_inner(), title, publication)?;

                    self.answer_entry_repository
                        .update(&form_update, &updated_entry)
                        .await?;

                    self.answer_entry_repository
                        .get(&form, answer_id)
                        .await?
                        .ok_or(AnswerNotFound)?
                }
            },
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

        let author_disclosure = form.answer_settings().author_disclosure_for(&actor_ref);
        self.build_answer_details(actor, form_id, form_answer, author_disclosure, labels)
            .await
    }
}

fn validate_related_answer_ids(
    answer_id: AnswerId,
    related_answer_ids: Vec<AnswerId>,
) -> Result<Vec<AnswerId>, DomainError> {
    let mut relations = std::collections::HashSet::new();

    for related_answer_id in &related_answer_ids {
        let relation = AnswerRelation::new(answer_id, *related_answer_id)?;
        if !relations.insert(relation) {
            return Err(DomainError::InvalidEntity {
                message: "duplicate related answer ids are not allowed".to_string(),
            });
        }
    }

    Ok(related_answer_ids)
}

fn answer_submitted_event(
    actor: AnswerSubmissionActor,
    form: &Allowed<ActiveForm, Read>,
    answer: &Allowed<AnswerEntry, domain::types::authorization_guard::Create>,
) -> ApplicationEvent {
    let questions = form.questions().as_slice();
    let title = answer
        .title()
        .to_owned()
        .into_inner()
        .map(|title| EventDetail::new("回答タイトル", title.into_inner()));
    let contents = answer.contents().iter().map(|content| {
        let question_title = questions
            .iter()
            .find(|question| question.id() == content.question_id)
            .map(|question| question.title().as_str().to_owned())
            .unwrap_or_else(|| "不明な質問".to_string());
        EventDetail::new(question_title, content.answer.to_owned())
    });

    ApplicationEvent::AnswerSubmitted {
        actor,
        form_id: form.id().to_string(),
        form_title: form.title().as_str().to_owned(),
        answer_id: answer.id().to_string(),
        details: title.into_iter().chain(contents).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use domain::{
        account::models::Role,
        form::{
            FormSubmissionRestriction, FormSubmissionRestrictionReason,
            answer::{AnswerLabelId, FormAnswerContentId},
            models::{
                AnswerAuthorPublicationPolicy, AnswerSettings, DefaultAnswerTitle,
                DiscordWebhookUrl, FormDescription, FormTitle, QuestionSet,
            },
            question::Question,
        },
        pagination::PageLimit,
        repository::form::{
            answer_label_repository::AnswerLabelRepository,
            answer_relation_repository::{RelatedAnswerLifecycle, RelatedAnswerReference},
        },
        types::authorization_guard::{AuthorizationGuard, Create, Delete, Update},
    };
    use errors::domain::DomainError;
    use std::sync::Mutex;
    use types::non_empty_string::NonEmptyString;
    use types::non_empty_vec::NonEmptyVec;
    use uuid::Uuid;

    use crate::test_utils::repositories::FormUseCaseTestRepositories;

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

    #[derive(Default)]
    struct RecordingDiscordAnswerWebhookNotifier(Mutex<Vec<DiscordAnswerWebhookNotification>>);

    #[async_trait]
    impl DiscordAnswerWebhookNotifier for RecordingDiscordAnswerWebhookNotifier {
        async fn notify_answer_posted(&self, notification: DiscordAnswerWebhookNotification) {
            self.0.lock().unwrap().push(notification);
        }
    }

    impl RecordingDiscordAnswerWebhookNotifier {
        fn notifications(&self) -> Vec<DiscordAnswerWebhookNotification> {
            self.0.lock().unwrap().clone()
        }
    }

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

    fn form_with_default_answer_title(allow_temporary_answers: bool) -> ActiveForm {
        sample_form().change_answer_settings(
            AnswerSettings::default()
                .change_default_answer_title(DefaultAnswerTitle::new(Some(
                    "$form_name".to_string().try_into().unwrap(),
                )))
                .change_allow_temporary_answers(allow_temporary_answers),
        )
    }

    fn answer_to(form: &ActiveForm) -> FormAnswerContent {
        FormAnswerContent {
            id: FormAnswerContentId::new(),
            question_id: (*form.questions().as_slice()[0].id()).into(),
            answer: "answer".to_string(),
        }
    }

    async fn only_posted_answer_title(
        repositories: &FormUseCaseTestRepositories,
        form_id: FormId,
    ) -> String {
        let administrator = active_user("admin", Role::Administrator);
        let form = repositories
            .active_form_repository
            .get(form_id)
            .await
            .unwrap()
            .unwrap()
            .try_read(Actor::from(administrator))
            .unwrap();
        let answers = repositories
            .answer_entry_repository
            .list_by_form(&form, PageRequest::first(PageLimit::default_limit()))
            .await
            .unwrap();

        answers.items()[0]
            .title()
            .to_owned()
            .into_inner()
            .unwrap()
            .into_inner()
    }

    #[tokio::test]
    async fn post_answers_uses_current_form_title_for_generated_title() {
        let form = form_with_default_answer_title(false);
        let form_id = *form.id();
        let answer = answer_to(&form);
        let user = active_user("user", Role::StandardUser);
        let repositories = FormUseCaseTestRepositories::with_active_forms(vec![form]);
        let empty_answer_label_repository = EmptyAnswerLabelRepository;
        let publisher = RecordingPublisher::default();
        let usecase = AnswerUseCase {
            active_form_repository: &repositories.active_form_repository,
            answer_label_repository: &empty_answer_label_repository,
            user_repository: &repositories.user_repository,
            form_submission_restriction_repository: &repositories
                .form_submission_restriction_repository,
            answer_entry_repository: &repositories.answer_entry_repository,
            answer_relation_repository: &repositories.answer_relation_repository,
            discord_answer_webhook_notifier: None,
            application_event_publisher: Some(&publisher),
        };

        usecase
            .post_answers(user, form_id, vec![answer])
            .await
            .unwrap();

        assert_eq!(
            only_posted_answer_title(&repositories, form_id).await,
            "Form"
        );
        assert!(matches!(
            publisher.events().as_slice(),
            [ApplicationEvent::AnswerSubmitted { actor, details, .. }]
                if matches!(actor, AnswerSubmissionActor::Identified(ApplicationActor {
                    display_name,
                    account_id: Some(_),
                }) if display_name == "user")
                    && details.iter().any(|detail| detail.value == "answer")
        ));
    }

    #[tokio::test]
    async fn post_temporary_answers_uses_current_form_title_for_generated_title() {
        // This test is the only usecase test that enables the form-specific notifier, which
        // constructs a frontend link from the process configuration.
        unsafe { std::env::set_var("FRONTEND_URL", "https://example.com") };
        let form = form_with_default_answer_title(true);
        let settings = form.settings().clone().change_discord_webhook_url(
            DiscordWebhookUrl::try_new(Some(
                NonEmptyString::try_new("https://discord.com/api/webhooks/123/token".to_string())
                    .unwrap(),
            ))
            .unwrap(),
        );
        let form = form.change_settings(settings);
        let form_id = *form.id();
        let answer = answer_to(&form);
        let repositories = FormUseCaseTestRepositories::with_active_forms(vec![form]);
        let empty_answer_label_repository = EmptyAnswerLabelRepository;
        let publisher = RecordingPublisher::default();
        let notifier = RecordingDiscordAnswerWebhookNotifier::default();
        let usecase = AnswerUseCase {
            active_form_repository: &repositories.active_form_repository,
            answer_label_repository: &empty_answer_label_repository,
            user_repository: &repositories.user_repository,
            form_submission_restriction_repository: &repositories
                .form_submission_restriction_repository,
            answer_entry_repository: &repositories.answer_entry_repository,
            answer_relation_repository: &repositories.answer_relation_repository,
            discord_answer_webhook_notifier: Some(&notifier),
            application_event_publisher: Some(&publisher),
        };

        usecase
            .post_temporary_answers(
                TemporaryAnswerAuthor::new("temporary user".to_string(), "contact".to_string()),
                form_id,
                vec![answer],
            )
            .await
            .unwrap();

        assert_eq!(
            only_posted_answer_title(&repositories, form_id).await,
            "Form"
        );
        assert!(matches!(
            publisher.events().as_slice(),
            [ApplicationEvent::AnswerSubmitted { actor, details, .. }]
                if matches!(actor, AnswerSubmissionActor::Identified(ApplicationActor {
                    display_name,
                    account_id: None,
                }) if display_name == "temporary user")
                    && details.iter().all(|detail| !detail.value.contains("contact"))
        ));
        assert!(matches!(
            notifier.notifications().as_slice(),
            [notification]
                if notification.fields.iter().all(|field| !field.value.contains("contact"))
                    && notification.fields.iter().any(|field|
                        field.name == "回答者" && field.value == "temporary user")
        ));
    }

    #[tokio::test]
    async fn hidden_author_is_removed_from_title_and_both_discord_notifications() {
        unsafe { std::env::set_var("FRONTEND_URL", "https://example.com") };
        let form = sample_form()
            .change_answer_settings(
                AnswerSettings::default()
                    .change_default_answer_title(DefaultAnswerTitle::new(Some(
                        "$username".to_string().try_into().unwrap(),
                    )))
                    .change_author_publication_policy(AnswerAuthorPublicationPolicy::Hide),
            )
            .change_settings(
                domain::form::models::FormSettings::new().change_discord_webhook_url(
                    DiscordWebhookUrl::try_new(Some(
                        "https://discord.com/api/webhooks/123/token"
                            .to_string()
                            .try_into()
                            .unwrap(),
                    ))
                    .unwrap(),
                ),
            );
        let form_id = *form.id();
        let answer = answer_to(&form);
        let user = active_user("secret user", Role::StandardUser);
        let repositories = FormUseCaseTestRepositories::with_active_forms(vec![form]);
        let labels = EmptyAnswerLabelRepository;
        let publisher = RecordingPublisher::default();
        let notifier = RecordingDiscordAnswerWebhookNotifier::default();
        let usecase = AnswerUseCase {
            active_form_repository: &repositories.active_form_repository,
            answer_label_repository: &labels,
            user_repository: &repositories.user_repository,
            form_submission_restriction_repository: &repositories
                .form_submission_restriction_repository,
            answer_entry_repository: &repositories.answer_entry_repository,
            answer_relation_repository: &repositories.answer_relation_repository,
            discord_answer_webhook_notifier: Some(&notifier),
            application_event_publisher: Some(&publisher),
        };

        usecase
            .post_answers(user, form_id, vec![answer])
            .await
            .unwrap();

        assert_eq!(
            only_posted_answer_title(&repositories, form_id).await,
            "匿名"
        );
        assert!(matches!(
            publisher.events().as_slice(),
            [ApplicationEvent::AnswerSubmitted {
                actor: AnswerSubmissionActor::AuthorHidden,
                ..
            }]
        ));
        assert!(matches!(
            notifier.notifications().as_slice(),
            [notification]
                if notification.fields.iter().any(|field|
                    field.name == "回答者" && field.value == "回答者は非公開です")
                    && notification.fields.iter().all(|field|
                        !field.value.contains("secret user"))
        ));
    }

    #[tokio::test]
    async fn hidden_answer_author_is_anonymous_to_its_author_and_identified_to_administrator() {
        let form = sample_form().change_answer_settings(
            AnswerSettings::default()
                .change_author_publication_policy(AnswerAuthorPublicationPolicy::Hide),
        );
        let form_id = *form.id();
        let author = active_user("answer author", Role::StandardUser);
        let administrator = active_user("administrator", Role::Administrator);
        let answer = AnswerEntry::new(
            form_id,
            AnswerAuthor::AuthenticatedUser(*author.id()),
            AnswerTitle::default(),
            PostedAnswerContents::try_new(form.questions().as_slice(), vec![answer_to(&form)])
                .unwrap(),
        );
        let answer_id = *answer.id();
        let mut repositories = FormUseCaseTestRepositories::with_active_forms(vec![form]);
        repositories.answer_entry_repository =
            crate::test_utils::repositories::InMemoryAnswerEntryRepository::new(vec![answer]);
        repositories.user_repository.save_user(author.clone());
        let labels = EmptyAnswerLabelRepository;
        let usecase = AnswerUseCase {
            active_form_repository: &repositories.active_form_repository,
            answer_label_repository: &labels,
            user_repository: &repositories.user_repository,
            form_submission_restriction_repository: &repositories
                .form_submission_restriction_repository,
            answer_entry_repository: &repositories.answer_entry_repository,
            answer_relation_repository: &repositories.answer_relation_repository,
            discord_answer_webhook_notifier: None,
            application_event_publisher: None,
        };

        let answer_for_author = usecase
            .get_answers(form_id, answer_id, &author)
            .await
            .unwrap();
        let answer_for_administrator = usecase
            .get_answers(form_id, answer_id, &administrator)
            .await
            .unwrap();

        assert!(matches!(
            answer_for_author.answer.author,
            PublishedAnswerAuthor::Anonymous
        ));
        assert!(matches!(
            answer_for_administrator.answer.author,
            PublishedAnswerAuthor::AuthenticatedUser(user)
                if user.id() == author.id() && user.name() == "answer author"
        ));
    }

    #[tokio::test]
    async fn administrator_can_make_an_answer_private_and_third_parties_cannot_read_it() {
        let form = sample_form().change_answer_settings(
            AnswerSettings::default()
                .change_visibility(domain::form::models::AnswerVisibility::PUBLIC),
        );
        let form_id = *form.id();
        let author = active_user("answer author", Role::StandardUser);
        let administrator = active_user("administrator", Role::Administrator);
        let third_party = active_user("third party", Role::StandardUser);
        let answer = AnswerEntry::new(
            form_id,
            AnswerAuthor::AuthenticatedUser(*author.id()),
            AnswerTitle::default(),
            PostedAnswerContents::try_new(form.questions().as_slice(), vec![answer_to(&form)])
                .unwrap(),
        );
        let answer_id = *answer.id();
        let mut repositories = FormUseCaseTestRepositories::with_active_forms(vec![form]);
        repositories.answer_entry_repository =
            crate::test_utils::repositories::InMemoryAnswerEntryRepository::new(vec![answer]);
        repositories.user_repository.save_user(author);
        let labels = EmptyAnswerLabelRepository;
        let usecase = AnswerUseCase {
            active_form_repository: &repositories.active_form_repository,
            answer_label_repository: &labels,
            user_repository: &repositories.user_repository,
            form_submission_restriction_repository: &repositories
                .form_submission_restriction_repository,
            answer_entry_repository: &repositories.answer_entry_repository,
            answer_relation_repository: &repositories.answer_relation_repository,
            discord_answer_webhook_notifier: None,
            application_event_publisher: None,
        };

        let updated = usecase
            .update_answer_meta(
                form_id,
                answer_id,
                &administrator,
                None,
                Some(AnswerPublication::PRIVATE),
                None,
            )
            .await
            .unwrap();

        assert_eq!(updated.answer.publication, AnswerPublication::PRIVATE);
        assert!(matches!(
            usecase.get_answers(form_id, answer_id, &third_party).await,
            Err(Error::Domain {
                source: DomainError::Forbidden
            })
        ));
    }

    #[tokio::test]
    async fn update_answer_meta_replaces_related_answers_only_when_the_command_is_present() {
        let form = sample_form();
        let form_id = *form.id();
        let administrator = active_user("administrator", Role::Administrator);
        let answer = AnswerEntry::new(
            form_id,
            AnswerAuthor::Temporary(TemporaryAnswerAuthor::new(
                "author".to_string(),
                "contact".to_string(),
            )),
            AnswerTitle::default(),
            PostedAnswerContents::try_new(form.questions().as_slice(), vec![answer_to(&form)])
                .unwrap(),
        );
        let answer_id = *answer.id();
        let mut repositories = FormUseCaseTestRepositories::with_active_forms(vec![form]);
        repositories.answer_entry_repository =
            crate::test_utils::repositories::InMemoryAnswerEntryRepository::new(vec![answer]);
        let labels = EmptyAnswerLabelRepository;
        let usecase = AnswerUseCase {
            active_form_repository: &repositories.active_form_repository,
            answer_label_repository: &labels,
            user_repository: &repositories.user_repository,
            form_submission_restriction_repository: &repositories
                .form_submission_restriction_repository,
            answer_entry_repository: &repositories.answer_entry_repository,
            answer_relation_repository: &repositories.answer_relation_repository,
            discord_answer_webhook_notifier: None,
            application_event_publisher: None,
        };

        usecase
            .update_answer_meta(form_id, answer_id, &administrator, None, None, None)
            .await
            .unwrap();
        assert_eq!(
            repositories
                .answer_relation_repository
                .replaced_targets(answer_id),
            None
        );

        let first_target = Uuid::new_v4().into();
        let second_target = Uuid::new_v4().into();
        usecase
            .update_answer_meta(
                form_id,
                answer_id,
                &administrator,
                None,
                None,
                Some(vec![first_target, second_target]),
            )
            .await
            .unwrap();
        assert_eq!(
            repositories
                .answer_relation_repository
                .replaced_targets(answer_id),
            Some(vec![first_target, second_target])
        );

        usecase
            .update_answer_meta(form_id, answer_id, &administrator, None, None, Some(vec![]))
            .await
            .unwrap();
        assert_eq!(
            repositories
                .answer_relation_repository
                .replaced_targets(answer_id),
            Some(vec![])
        );
    }

    #[tokio::test]
    async fn update_answer_meta_rejects_invalid_related_answers_before_persisting_metadata() {
        let form = sample_form();
        let form_id = *form.id();
        let administrator = active_user("administrator", Role::Administrator);
        let answer = AnswerEntry::new(
            form_id,
            AnswerAuthor::Temporary(TemporaryAnswerAuthor::new(
                "author".to_string(),
                "contact".to_string(),
            )),
            AnswerTitle::default(),
            PostedAnswerContents::try_new(form.questions().as_slice(), vec![answer_to(&form)])
                .unwrap(),
        );
        let answer_id = *answer.id();
        let mut repositories = FormUseCaseTestRepositories::with_active_forms(vec![form]);
        repositories.answer_entry_repository =
            crate::test_utils::repositories::InMemoryAnswerEntryRepository::new(vec![answer]);
        let labels = EmptyAnswerLabelRepository;
        let usecase = AnswerUseCase {
            active_form_repository: &repositories.active_form_repository,
            answer_label_repository: &labels,
            user_repository: &repositories.user_repository,
            form_submission_restriction_repository: &repositories
                .form_submission_restriction_repository,
            answer_entry_repository: &repositories.answer_entry_repository,
            answer_relation_repository: &repositories.answer_relation_repository,
            discord_answer_webhook_notifier: None,
            application_event_publisher: None,
        };

        let self_relation = usecase
            .update_answer_meta(
                form_id,
                answer_id,
                &administrator,
                Some(AnswerTitle::new(Some(
                    "changed".to_string().try_into().unwrap(),
                ))),
                Some(AnswerPublication::PRIVATE),
                Some(vec![answer_id]),
            )
            .await;
        assert!(matches!(
            self_relation,
            Err(Error::Domain {
                source: DomainError::InvalidEntity { .. }
            })
        ));
        let unchanged_answer = usecase
            .get_answers(form_id, answer_id, &administrator)
            .await
            .unwrap();
        assert_eq!(unchanged_answer.answer.title, AnswerTitle::default());
        assert_eq!(
            unchanged_answer.answer.publication,
            AnswerPublication::PUBLIC
        );

        let archived_or_missing_target = Uuid::new_v4().into();
        repositories
            .answer_relation_repository
            .mark_target_archived_or_missing(archived_or_missing_target);
        let archived_or_missing_relation = usecase
            .update_answer_meta(
                form_id,
                answer_id,
                &administrator,
                Some(AnswerTitle::new(Some(
                    "changed again".to_string().try_into().unwrap(),
                ))),
                Some(AnswerPublication::PRIVATE),
                Some(vec![archived_or_missing_target]),
            )
            .await;
        assert!(matches!(
            archived_or_missing_relation,
            Err(Error::Domain {
                source: DomainError::InvalidEntity { .. }
            })
        ));
        let still_unchanged_answer = usecase
            .get_answers(form_id, answer_id, &administrator)
            .await
            .unwrap();
        assert_eq!(still_unchanged_answer.answer.title, AnswerTitle::default());
        assert_eq!(
            still_unchanged_answer.answer.publication,
            AnswerPublication::PUBLIC
        );

        let target = Uuid::new_v4().into();
        let duplicate_relation = usecase
            .update_answer_meta(
                form_id,
                answer_id,
                &administrator,
                None,
                None,
                Some(vec![target, target]),
            )
            .await;
        assert!(matches!(
            duplicate_relation,
            Err(Error::Domain {
                source: DomainError::InvalidEntity { .. }
            })
        ));
        assert_eq!(
            repositories
                .answer_relation_repository
                .replaced_targets(answer_id),
            None
        );
    }

    #[tokio::test]
    async fn related_answers_are_visible_to_standard_users_only_when_active_and_readable() {
        let form = sample_form().change_answer_settings(
            AnswerSettings::default()
                .change_visibility(domain::form::models::AnswerVisibility::PUBLIC),
        );
        let form_id = *form.id();
        let source = AnswerEntry::new(
            form_id,
            AnswerAuthor::Temporary(TemporaryAnswerAuthor::new(
                "source".to_string(),
                "contact".to_string(),
            )),
            AnswerTitle::default(),
            PostedAnswerContents::try_new(form.questions().as_slice(), vec![answer_to(&form)])
                .unwrap(),
        );
        let readable_target = AnswerEntry::new(
            form_id,
            AnswerAuthor::Temporary(TemporaryAnswerAuthor::new(
                "target".to_string(),
                "contact".to_string(),
            )),
            AnswerTitle::default(),
            PostedAnswerContents::try_new(form.questions().as_slice(), vec![answer_to(&form)])
                .unwrap(),
        );
        let private_target = AnswerEntry::new(
            form_id,
            AnswerAuthor::Temporary(TemporaryAnswerAuthor::new(
                "private target".to_string(),
                "contact".to_string(),
            )),
            AnswerTitle::default(),
            PostedAnswerContents::try_new(form.questions().as_slice(), vec![answer_to(&form)])
                .unwrap(),
        )
        .change_publication(AnswerPublication::PRIVATE);
        let source_id = *source.id();
        let readable_relation = RelatedAnswerReference {
            form_id,
            answer_id: *readable_target.id(),
            lifecycle: RelatedAnswerLifecycle::Active,
        };
        let unreadable_relation = RelatedAnswerReference {
            form_id,
            answer_id: *private_target.id(),
            lifecycle: RelatedAnswerLifecycle::Active,
        };
        let archived_relation = RelatedAnswerReference {
            form_id: Uuid::new_v4().into(),
            answer_id: Uuid::new_v4().into(),
            lifecycle: RelatedAnswerLifecycle::Archived,
        };
        let mut repositories = FormUseCaseTestRepositories::with_active_forms(vec![form]);
        repositories.answer_entry_repository =
            crate::test_utils::repositories::InMemoryAnswerEntryRepository::new(vec![
                source,
                readable_target,
                private_target,
            ]);
        repositories.answer_relation_repository.set_related_answers(
            source_id,
            vec![readable_relation, unreadable_relation, archived_relation],
        );
        let labels = EmptyAnswerLabelRepository;
        let usecase = AnswerUseCase {
            active_form_repository: &repositories.active_form_repository,
            answer_label_repository: &labels,
            user_repository: &repositories.user_repository,
            form_submission_restriction_repository: &repositories
                .form_submission_restriction_repository,
            answer_entry_repository: &repositories.answer_entry_repository,
            answer_relation_repository: &repositories.answer_relation_repository,
            discord_answer_webhook_notifier: None,
            application_event_publisher: None,
        };
        let standard_user = active_user("standard", Role::StandardUser);
        let administrator = active_user("administrator", Role::Administrator);

        let standard_answer = usecase
            .get_answers(form_id, source_id, &standard_user)
            .await
            .unwrap();
        let administrator_answer = usecase
            .get_answers(form_id, source_id, &administrator)
            .await
            .unwrap();

        assert_eq!(standard_answer.related_answers, vec![readable_relation]);
        assert_eq!(
            administrator_answer.related_answers,
            vec![readable_relation, unreadable_relation, archived_relation]
        );
    }

    #[tokio::test]
    async fn post_answers_rejects_user_with_active_form_submission_restriction() {
        let form = sample_form();
        let user = active_user("user", Role::StandardUser);
        let now = Utc::now();
        let restriction = FormSubmissionRestriction::new(
            *user.id(),
            FormSubmissionRestrictionReason::new("spam".to_string().try_into().unwrap()),
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
            .form_submission_restriction_repository
            .save_form_submission_restriction(restriction);
        let empty_answer_label_repository = EmptyAnswerLabelRepository;
        let usecase = AnswerUseCase {
            active_form_repository: &repositories.active_form_repository,
            answer_label_repository: &empty_answer_label_repository,
            user_repository: &repositories.user_repository,
            form_submission_restriction_repository: &repositories
                .form_submission_restriction_repository,
            answer_entry_repository: &repositories.answer_entry_repository,
            answer_relation_repository: &repositories.answer_relation_repository,
            discord_answer_webhook_notifier: None,
            application_event_publisher: None,
        };

        let result = usecase.post_answers(user, *form.id(), vec![answer]).await;

        assert_eq!(
            result,
            Err(DomainError::SubmissionRestricted {
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
