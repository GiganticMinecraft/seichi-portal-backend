use sea_orm_migration::prelude::*;
use crate::m20221211_211233_form_questions::FormQuestions;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FormChoices::FormChoices)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FormChoices::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(FormChoices::QuestionId).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-question_id")
                            .from(FormChoices::FormChoices, FormChoices::QuestionId)
                            .to(FormQuestions::FormQuestions, FormQuestions::QuestionId)
                    )
                    .col(ColumnDef::new(FormChoices::Choice).string().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(FormChoices::FormChoices).to_owned())
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum FormChoices {
    FormChoices,
    Id,
    QuestionId,
    Choice,
}
