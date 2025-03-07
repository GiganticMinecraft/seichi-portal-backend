use std::sync::Arc;

use domain::{
    form::{
        answer::service::AnswerEntryAuthorizationContext,
        comment::service::CommentAuthorizationContext,
    },
    repository::{
        form::{answer_repository::AnswerRepository, form_repository::FormRepository},
        search_repository::SearchRepository,
    },
    search::models::SearchableFields,
    user::models::User,
};
use errors::{
    Error,
    usecase::UseCaseError::{AnswerNotFound, FormNotFound},
};
use futures::{future::try_join_all, try_join};
use tokio::sync::{Notify, mpsc::Receiver};

use crate::dto::CrossSearchDto;

pub struct SearchUseCase<
    'a,
    SearchRepo: SearchRepository,
    AnswerRepo: AnswerRepository,
    FormRepo: FormRepository,
> {
    pub repository: &'a SearchRepo,
    pub answer_repository: &'a AnswerRepo,
    pub form_repository: &'a FormRepo,
}

impl<R1: SearchRepository, R2: AnswerRepository, R3: FormRepository> SearchUseCase<'_, R1, R2, R3> {
    pub async fn cross_search(&self, actor: &User, query: String) -> Result<CrossSearchDto, Error> {
        let (forms, users, label_for_forms, label_for_answers, answers, comments) = try_join!(
            self.repository.search_forms(&query),
            self.repository.search_users(&query),
            self.repository.search_labels_for_forms(&query),
            self.repository.search_labels_for_answers(&query),
            self.repository.search_answers(&query),
            self.repository.search_comments(&query)
        )?;

        let forms = forms
            .into_iter()
            .flat_map(|guard| guard.try_into_read(actor))
            .collect::<Vec<_>>();

        let users = users
            .into_iter()
            .flat_map(|guard| guard.try_into_read(actor))
            .collect::<Vec<_>>();

        let label_for_forms = label_for_forms
            .into_iter()
            .flat_map(|guard| guard.try_into_read(actor))
            .collect::<Vec<_>>();

        let label_for_answers = label_for_answers
            .into_iter()
            .flat_map(|guard| guard.try_into_read(actor))
            .collect::<Vec<_>>();

        let comment_futs = comments
            .into_iter()
            .map(|guard| async {
                let context = guard
                    .create_context(|comment| {
                        let answer_id = comment.answer_id().to_owned();

                        async move {
                            let answer_entry_guard = self
                                .answer_repository
                                .get_answer(answer_id)
                                .await?
                                .ok_or(Error::from(AnswerNotFound))?;

                            let answer_entry_context = answer_entry_guard
                                .create_context(|entry| {
                                    let form_id = entry.form_id().to_owned();

                                    async move {
                                        let form_guard = self
                                            .form_repository
                                            .get(form_id)
                                            .await?
                                            .ok_or(Error::from(FormNotFound))?;

                                        let form = form_guard.try_read(actor)?;
                                        let form_settings = form.settings();

                                        Ok(AnswerEntryAuthorizationContext {
                                            form_visibility: form_settings.visibility().to_owned(),
                                            response_period: form_settings
                                                .answer_settings()
                                                .response_period()
                                                .to_owned(),
                                            answer_visibility: form_settings
                                                .answer_settings()
                                                .visibility()
                                                .to_owned(),
                                        })
                                    }
                                })
                                .await?;

                            Ok(CommentAuthorizationContext {
                                related_answer_entry_guard: answer_entry_guard,
                                related_answer_entry_guard_context: answer_entry_context,
                            })
                        }
                    })
                    .await?;

                guard
                    .try_into_read(actor, &context)
                    .map_err(Into::<Error>::into)
            })
            .collect::<Vec<_>>();

        let comments = try_join_all(comment_futs).await?;

        Ok(CrossSearchDto {
            forms,
            users,
            label_for_forms,
            label_for_answers,
            answers,
            comments,
        })
    }

    pub async fn start_sync(
        &self,
        receiver: Receiver<SearchableFields>,
        shutdown_notifier: Arc<Notify>,
    ) -> Result<(), Error> {
        self.repository
            .start_sync(receiver, shutdown_notifier)
            .await
    }
}
