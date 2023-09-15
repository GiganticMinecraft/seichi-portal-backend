use sea_orm_migration::prelude::*;

use crate::m20220101_000001_create_table::FormMetaDataTable;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(DefaultAnswerTitlesTable::DefaultAnswerTitles)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(DefaultAnswerTitlesTable::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(DefaultAnswerTitlesTable::FormId)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-form_key_default_answer_title")
                            .from(
                                DefaultAnswerTitlesTable::DefaultAnswerTitles,
                                DefaultAnswerTitlesTable::FormId,
                            )
                            .to(FormMetaDataTable::FormMetaData, FormMetaDataTable::Id),
                    )
                    .col(
                        ColumnDef::new(DefaultAnswerTitlesTable::Title)
                            .string()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(DefaultAnswerTitlesTable::DefaultAnswerTitles)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum DefaultAnswerTitlesTable {
    DefaultAnswerTitles,
    Id,
    FormId,
    Title,
}
