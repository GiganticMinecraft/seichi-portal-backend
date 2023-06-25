pub mod form_repository_impl;

use std::{fmt::Debug, sync::Arc};

use domain::repository::Repositories;

use crate::database::{components::DatabaseComponents, connection::ConnectionPool};

pub type RealInfrastructureRepository = SharedRepository<ConnectionPool>;

#[derive(Debug, Clone)]
pub struct SharedRepository<Client: DatabaseComponents + Debug + 'static>(Arc<Repository<Client>>);

#[derive(Debug)]
pub struct Repository<Client: DatabaseComponents + Debug + 'static> {
    pub(crate) client: Client,
}

impl<Client: DatabaseComponents + Debug + 'static> Repository<Client> {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub fn into_shared(self) -> SharedRepository<Client> {
        SharedRepository(Arc::new(self))
    }
}

impl<Client: DatabaseComponents + Debug + 'static> Repositories for SharedRepository<Client> {
    type ConcreteFormRepository = Repository<Client>;

    fn form_repository(&self) -> &Self::ConcreteFormRepository {
        &self.0
    }
}
