use crate::m20221211_211233_form_questions::FormQuestionsTable;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FormChoicesTable::FormChoices)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FormChoicesTable::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(FormChoicesTable::QuestionId)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-question_id")
                            .from(FormChoicesTable::FormChoices, FormChoicesTable::QuestionId)
                            .to(
                                FormQuestionsTable::FormQuestions,
                                FormQuestionsTable::QuestionId,
                            ),
                    )
                    .col(ColumnDef::new(FormChoicesTable::Choice).string().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(FormChoicesTable::FormChoices)
                    .to_owned(),
            )
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum FormChoicesTable {
    FormChoices,
    Id,
    QuestionId,
    Choice,
}
