pub mod form_repository_impls;
pub mod notification_repository_impl;
pub mod search_repository_impl;
pub mod user_repository_impl;

use std::sync::Arc;

use domain::repository::Repositories;

use crate::database::{components::DatabaseComponents, connection::ConnectionPool};

pub type RealInfrastructureRepository = SharedRepository<ConnectionPool>;

#[derive(Clone)]
pub struct SharedRepository<Client: DatabaseComponents + 'static>(Arc<Repository<Client>>);

pub struct Repository<Client: DatabaseComponents + 'static> {
    pub(crate) client: Client,
}

impl<Client: DatabaseComponents + 'static> Repository<Client> {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub fn into_shared(self) -> SharedRepository<Client> {
        SharedRepository(Arc::new(self))
    }
}

impl<Client: DatabaseComponents + 'static> Repositories for SharedRepository<Client> {
    type ConcreteAnswerLabelRepository = Repository<Client>;
    type ConcreteFormAnswerRepository = Repository<Client>;
    type ConcreteFormCommentRepository = Repository<Client>;
    type ConcreteFormLabelRepository = Repository<Client>;
    type ConcreteFormMessageRepository = Repository<Client>;
    type ConcreteFormQuestionRepository = Repository<Client>;
    type ConcreteFormRepository = Repository<Client>;
    type ConcreteNotificationRepository = Repository<Client>;
    type ConcreteSearchRepository = Repository<Client>;
    type ConcreteUserRepository = Repository<Client>;

    fn form_repository(&self) -> &Self::ConcreteFormRepository {
        &self.0
    }

    fn form_answer_repository(&self) -> &Self::ConcreteFormAnswerRepository {
        &self.0
    }

    fn answer_label_repository(&self) -> &Self::ConcreteAnswerLabelRepository {
        &self.0
    }

    fn form_question_repository(&self) -> &Self::ConcreteFormQuestionRepository {
        &self.0
    }

    fn form_message_repository(&self) -> &Self::ConcreteFormMessageRepository {
        &self.0
    }

    fn form_comment_repository(&self) -> &Self::ConcreteFormCommentRepository {
        &self.0
    }

    fn form_label_repository(&self) -> &Self::ConcreteFormLabelRepository {
        &self.0
    }

    fn notification_repository(&self) -> &Self::ConcreteNotificationRepository {
        &self.0
    }

    fn user_repository(&self) -> &Self::ConcreteUserRepository {
        &self.0
    }

    fn search_repository(&self) -> &Self::ConcreteSearchRepository {
        &self.0
    }
}
