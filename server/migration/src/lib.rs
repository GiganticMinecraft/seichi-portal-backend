pub use sea_orm_migration::prelude::*;

mod m20220101_000001_create_table;
mod m20221127_173808_create_form_question_type_enum_table;
mod m20221211_211233_form_questions;
mod m20230219_143118_create_form_choices;
mod m20230312_132648_question_type_seeding;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_table::Migration),
            Box::new(m20221127_173808_create_form_question_type_enum_table::Migration),
            Box::new(m20221211_211233_form_questions::Migration),
            Box::new(m20230219_143118_create_form_choices::Migration),
            Box::new(m20230312_132648_question_type_seeding::Migration),
        ]
    }
}
