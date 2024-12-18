use async_trait::async_trait;
use domain::{
    form::{
        answer::models::{AnswerLabel, FormAnswerContent},
        models::{Form, FormLabel},
    },
    repository::search_repository::SearchRepository,
    search::models::Comment,
    user::models::User,
};
use errors::Error;

use crate::{
    database::components::{DatabaseComponents, SearchDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> SearchRepository for Repository<Client> {
    async fn search_users(&self, query: &str) -> Result<Vec<User>, Error> {
        self.client
            .search()
            .search_users(query)
            .await
            .map_err(Into::into)
    }

    async fn search_forms(&self, query: &str) -> Result<Vec<Form>, Error> {
        self.client
            .search()
            .search_forms(query)
            .await
            .map_err(Into::into)
    }

    async fn search_labels_for_forms(&self, query: &str) -> Result<Vec<FormLabel>, Error> {
        self.client
            .search()
            .search_labels_for_forms(query)
            .await
            .map_err(Into::into)
    }

    async fn search_labels_for_answers(&self, query: &str) -> Result<Vec<AnswerLabel>, Error> {
        self.client
            .search()
            .search_labels_for_answers(query)
            .await
            .map_err(Into::into)
    }

    async fn search_answers(&self, query: &str) -> Result<Vec<FormAnswerContent>, Error> {
        self.client
            .search()
            .search_answers(query)
            .await
            .map_err(Into::into)
    }

    async fn search_comments(&self, query: &str) -> Result<Vec<Comment>, Error> {
        self.client
            .search()
            .search_comments(query)
            .await
            .map_err(Into::into)
    }
}
