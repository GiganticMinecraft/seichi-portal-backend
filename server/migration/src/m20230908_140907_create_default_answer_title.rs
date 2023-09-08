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
                    .table(DefaultAnswerTitleTable::DefaultAnswerTitle)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(DefaultAnswerTitleTable::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(DefaultAnswerTitleTable::FormId)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-form_key_default_answer_title")
                            .from(
                                DefaultAnswerTitleTable::DefaultAnswerTitle,
                                DefaultAnswerTitleTable::FormId,
                            )
                            .to(FormMetaDataTable::FormMetaData, FormMetaDataTable::Id),
                    )
                    .col(ColumnDef::new(DefaultAnswerTitleTable::Title).string())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(DefaultAnswerTitleTable::DefaultAnswerTitle)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum DefaultAnswerTitleTable {
    DefaultAnswerTitle,
    Id,
    FormId,
    Title,
}
