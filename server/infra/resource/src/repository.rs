pub mod form_repository_impls;
pub mod notification_repository_impl;
pub mod search_repository_impl;
pub mod user_repository_impl;

use std::sync::Arc;

use domain::repository::Repositories;

use domain::repository::health_check_repository::HealthCheckRepository;

use crate::database::{components::DatabaseComponents, connection::ConnectionPool};

pub type RealInfrastructureRepository = SharedRepository<ConnectionPool>;

#[derive(Clone)]
pub struct SharedRepository<Client: DatabaseComponents + 'static> {
    db: Arc<Repository<Client>>,
    health_check: Arc<dyn HealthCheckRepository + Send + Sync>,
}

pub struct Repository<Client: DatabaseComponents + 'static> {
    pub(crate) client: Client,
}

impl<Client: DatabaseComponents + 'static> Repository<Client> {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub fn into_shared(
        self,
        health_check: Arc<dyn HealthCheckRepository + Send + Sync>,
    ) -> SharedRepository<Client> {
        SharedRepository {
            db: Arc::new(self),
            health_check,
        }
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
        &self.db
    }

    fn form_answer_repository(&self) -> &Self::ConcreteFormAnswerRepository {
        &self.db
    }

    fn answer_label_repository(&self) -> &Self::ConcreteAnswerLabelRepository {
        &self.db
    }

    fn form_question_repository(&self) -> &Self::ConcreteFormQuestionRepository {
        &self.db
    }

    fn form_message_repository(&self) -> &Self::ConcreteFormMessageRepository {
        &self.db
    }

    fn form_comment_repository(&self) -> &Self::ConcreteFormCommentRepository {
        &self.db
    }

    fn form_label_repository(&self) -> &Self::ConcreteFormLabelRepository {
        &self.db
    }

    fn notification_repository(&self) -> &Self::ConcreteNotificationRepository {
        &self.db
    }

    fn user_repository(&self) -> &Self::ConcreteUserRepository {
        &self.db
    }

    fn search_repository(&self) -> &Self::ConcreteSearchRepository {
        &self.db
    }

    fn health_check_repository(&self) -> &(dyn HealthCheckRepository + Send + Sync) {
        &*self.health_check
    }
}
