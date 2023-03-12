use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(QuestionTypeEnumTable::QuestionTypes)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(QuestionTypeEnumTable::Id)
                            .integer()
                            .auto_increment()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(QuestionTypeEnumTable::QuestionType)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(QuestionTypeEnumTable::QuestionTypes)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
pub enum QuestionTypeEnumTable {
    QuestionTypes,
    Id,
    QuestionType,
}
