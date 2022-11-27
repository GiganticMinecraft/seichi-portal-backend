use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FormQuestions::FormQuestions)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FormQuestions::QuestionId)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(FormQuestions::FormId).integer().not_null())
                    .col(ColumnDef::new(FormQuestions::Title).string().not_null())
                    .col(
                        ColumnDef::new(FormQuestions::Description)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FormQuestions::AnswerType)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(FormQuestions::Choices).string())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(FormQuestions::FormQuestions).to_owned())
            .await
    }
}

#[derive(Iden)]
enum FormQuestions {
    FormQuestions,
    QuestionId,
    FormId,
    Title,
    Description,
    AnswerType,
    Choices,
}
