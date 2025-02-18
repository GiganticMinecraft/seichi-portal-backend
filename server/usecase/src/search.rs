use crate::dto::CrossSearchDto;
use domain::repository::search_repository::SearchRepository;
use domain::user::models::User;
use errors::Error;
use futures::try_join;

pub struct SearchUseCase<'a, SearchRepo: SearchRepository> {
    pub repository: &'a SearchRepo,
}

impl<R: SearchRepository> SearchUseCase<'_, R> {
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

        Ok(CrossSearchDto {
            forms,
            users,
            label_for_forms,
            label_for_answers,
            answers,
            comments,
        })
    }
}
