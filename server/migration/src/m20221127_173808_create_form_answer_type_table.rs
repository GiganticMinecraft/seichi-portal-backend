use entities::answer_types;
use sea_orm_migration::{prelude::*, sea_orm::EntityTrait};

use crate::sea_orm::ActiveValue::Set;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AnswerTypes::AnswerTypes)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AnswerTypes::AnswerType)
                            .string()
                            .not_null()
                            .unique_key()
                            .primary_key(),
                    )
                    .to_owned(),
            )
            .await?;

        let connection = manager.get_connection();

        let models = vec!["TEXT", "CHECKBOX", "PULLDOWN"]
            .into_iter()
            .map(|answer_type| answer_types::ActiveModel {
                answer_type: Set(answer_type.to_owned()),
            })
            .collect::<Vec<answer_types::ActiveModel>>();

        answer_types::Entity::insert_many(models)
            .exec(connection)
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AnswerTypes::AnswerTypes).to_owned())
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
pub enum AnswerTypes {
    AnswerTypes,
    AnswerType,
}
