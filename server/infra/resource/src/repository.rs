pub mod form_repository_impl;
mod search_repository_impl;
mod user_repository_impl;

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
    type ConcreteFormRepository = Repository<Client>;
    type ConcreteSearchRepository = Repository<Client>;
    type ConcreteUserRepository = Repository<Client>;

    fn form_repository(&self) -> &Self::ConcreteFormRepository {
        &self.0
    }

    fn user_repository(&self) -> &Self::ConcreteUserRepository {
        &self.0
    }

    fn search_repository(&self) -> &Self::ConcreteSearchRepository {
        &self.0
    }
}
