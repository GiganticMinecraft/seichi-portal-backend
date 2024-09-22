use domain::{repository::search_repository::SearchRepository, search::models::FullSearch};
use errors::Error;
use futures::try_join;

pub struct SearchUseCase<'a, SearchRepo: SearchRepository> {
    pub repository: &'a SearchRepo,
}

impl<R: SearchRepository> SearchUseCase<'_, R> {
    pub async fn full_search(&self, query: String) -> Result<FullSearch, Error> {
        let (forms, users, label_for_forms, label_for_answers, answers) = try_join!(
            self.repository.search_forms(query.to_owned()),
            self.repository.search_users(query.to_owned()),
            self.repository.search_labels_for_forms(query.to_owned()),
            self.repository.search_labels_for_answers(query.to_owned()),
            self.repository.search_answers(query.to_owned())
        )?;

        Ok(FullSearch {
            forms,
            users,
            label_for_forms,
            label_for_answers,
            answers,
        })
    }
}
