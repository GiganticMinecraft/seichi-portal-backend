use domain::{repository::search_repository::SearchRepository, search::models::CrossSearchResult};
use errors::Error;
use futures::try_join;

pub struct SearchUseCase<'a, SearchRepo: SearchRepository> {
    pub repository: &'a SearchRepo,
}

impl<R: SearchRepository> SearchUseCase<'_, R> {
    pub async fn cross_search(&self, query: String) -> Result<CrossSearchResult, Error> {
        let (forms, users, label_for_forms, label_for_answers, answers) = try_join!(
            self.repository.search_forms(query.to_owned()),
            self.repository.search_users(query.to_owned()),
            self.repository.search_labels_for_forms(query.to_owned()),
            self.repository.search_labels_for_answers(query.to_owned()),
            self.repository.search_answers(query.to_owned())
        )?;

        Ok(CrossSearchResult {
            forms,
            users,
            label_for_forms,
            label_for_answers,
            answers,
        })
    }
}
