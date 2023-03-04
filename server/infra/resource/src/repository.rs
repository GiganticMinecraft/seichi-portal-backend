pub mod form_repository_impl;

use crate::database::components::DatabaseComponents;
use crate::database::connection::ConnectionPool;
use domain::repository::Repositories;
use std::sync::Arc;

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

    fn form_repository(&self) -> &Self::ConcreteFormRepository {
        &self.0
    }
}
