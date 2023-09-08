pub use sea_orm_migration::prelude::*;

mod m20220101_000001_create_table;
mod m20221211_211233_form_questions;
mod m20230219_143118_create_form_choices;
mod m20230614_083950_crate_form_response_period_table;
mod m20230622_053919_create_form_webhook;
mod m20230811_062425_create_answer_tables;
mod m20230908_140907_create_default_answer_title;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_table::Migration),
            Box::new(m20221211_211233_form_questions::Migration),
            Box::new(m20230219_143118_create_form_choices::Migration),
            Box::new(m20230614_083950_crate_form_response_period_table::Migration),
            Box::new(m20230622_053919_create_form_webhook::Migration),
            Box::new(m20230811_062425_create_answer_tables::Migration),
            Box::new(m20230908_140907_create_default_answer_title::Migration),
        ]
    }
}
