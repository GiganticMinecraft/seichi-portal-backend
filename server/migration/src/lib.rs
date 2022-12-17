pub use sea_orm_migration::prelude::*;

mod m20220101_000001_create_table;
mod m20221127_173808_form_questions;
mod m20221211_211233_create_form_answer_type_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_table::Migration),
            Box::new(m20221127_173808_form_questions::Migration),
            Box::new(m20221211_211233_create_form_answer_type_table::Migration),
        ]
    }
}