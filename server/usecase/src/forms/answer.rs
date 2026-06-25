use domain::{
    form::{
        answer::{
            AnswerAuthor, AnswerEntry, AnswerId, AnswerLabel, AnswerTitle, FormAnswerContent,
            PostedAnswerContents,
        },
        models::{ActiveForm, FormId},
        service::DefaultAnswerTitleDomainService,
    },
    repository::form::{
        active_form_repository::ActiveFormRepository,
        answer_entry_repository::AnswerEntryRepository,
        answer_label_repository::AnswerLabelRepository,
    },
    repository::user_repository::UserRepository,
    types::authorization_guard::{Allowed, Read},
    user::models::{ActiveUser, Actor, TemporaryUser, User},
};
use errors::{
    Error,
    domain::DomainError,
    usecase::UseCaseError::{AnswerNotFound, FormNotFound},
};
use futures::{StreamExt, stream};

use crate::{
    forms::answer_webhook::{AnswerWebhookField, AnswerWebhookNotification, AnswerWebhookNotifier},
    models::AnswerDetails,
    user_reference_resolver::resolve_user_references,
};
use common::config::FRONTEND;

pub struct AnswerUseCase<
    'a,
    FormRepo: ActiveFormRepository,
    AnswerLabelRepo: AnswerLabelRepository,
    UserRepo: UserRepository,
    AnswerEntryRepo: AnswerEntryRepository,
> {
    pub active_form_repository: &'a FormRepo,
    pub answer_label_repository: &'a AnswerLabelRepo,
    pub user_repository: &'a UserRepo,
    pub answer_entry_repository: &'a AnswerEntryRepo,
    pub answer_webhook_notifier: Option<&'a dyn AnswerWebhookNotifier>,
}

impl<
    R1: ActiveFormRepository,
    R2: AnswerLabelRepository,
    R3: UserRepository,
    R4: AnswerEntryRepository,
> AnswerUseCase<'_, R1, R2, R3, R4>
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
        actor: &ActiveUser,
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
            AnswerAuthor::AuthenticatedUser(user_id) => User::ActiveUser(
                users
                    .get(user_id)
                    .cloned()
                    .ok_or(Error::from(errors::usecase::UseCaseError::UserNotFound))?,
            ),
            AnswerAuthor::TemporaryUser(temporary_user) => {
                User::TemporaryUser(temporary_user.clone())
            }
        };

        Ok(AnswerDetails {
            form_id,
            form_answer: form_answer.into_inner(),
            author,
            labels,
        })
    }

    async fn notify_answer_webhook(
        &self,
        form: &Allowed<ActiveForm, Read>,
        answer_entry: &Allowed<AnswerEntry, domain::types::authorization_guard::Create>,
        respondent: String,
    ) {
        let Some(notifier) = self.answer_webhook_notifier else {
            return;
        };
        let Some(webhook_url) = form
            .settings()
            .webhook_url(&Actor::System)
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

                AnswerWebhookField::new(question_title, content.answer.clone())
            })
            .collect::<Vec<_>>();
        let fields = [
            vec![
                AnswerWebhookField::new(
                    "フォーム名".to_string(),
                    form.title().to_owned().into_inner().into_inner(),
                ),
                AnswerWebhookField::new("タイトル".to_string(), answer_title),
                AnswerWebhookField::new("回答者".to_string(), respondent),
            ],
            answer_fields,
        ]
        .into_iter()
        .flatten()
        .collect();

        notifier
            .notify_answer_posted(AnswerWebhookNotification {
                webhook_url,
                answer_url,
                form_id,
                answer_id,
                fields,
            })
            .await;
    }

    pub async fn post_answers(
        &self,
        user: ActiveUser,
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

        let title = DefaultAnswerTitleDomainService::to_answer_title_from_questions(
            form.value()
                .answer_settings()
                .default_answer_title()
                .to_owned(),
            &questions,
            &posted_answers,
            user.name(),
        )?;

        let author = AnswerAuthor::AuthenticatedUser(*user.id());
        let answer_entry = form.try_accept_answer(author, title, posted_answers)?;

        self.answer_entry_repository
            .post(&form, &answer_entry)
            .await?;

        self.notify_answer_webhook(&form, &answer_entry, user.name().to_string())
            .await;

        Ok(())
    }

    pub async fn post_temporary_answers(
        &self,
        temporary_user: TemporaryUser,
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
        let author = AnswerAuthor::TemporaryUser(temporary_user);
        let answer_entry = form.try_accept_answer(author, title, posted_answers)?;

        self.answer_entry_repository
            .post(&form, &answer_entry)
            .await?;

        self.notify_answer_webhook(&form, &answer_entry, respondent)
            .await;

        Ok(())
    }

    pub async fn get_answers(
        &self,
        form_id: FormId,
        answer_id: AnswerId,
        user: &ActiveUser,
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
        actor: &ActiveUser,
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

    pub async fn get_all_answers(&self, user: &ActiveUser) -> Result<Vec<AnswerDetails>, Error> {
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
        actor: &ActiveUser,
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
