use crate::m20220101_000001_create_table::FormMetaDataTable;
use sea_orm_migration::prelude::*;

use crate::m20221211_211233_form_questions::FormQuestionsTable;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AnswersTable::Answers)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AnswersTable::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(AnswersTable::FormId).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-form-id-from-answers")
                            .from(AnswersTable::Answers, AnswersTable::FormId)
                            .to(FormMetaDataTable::FormMetaData, FormMetaDataTable::Id),
                    )
                    .col(ColumnDef::new(AnswersTable::User).uuid().not_null())
                    .col(
                        ColumnDef::new(AnswersTable::Title)
                            .string()
                            .not_null()
                            .default("未設定"),
                    )
                    .col(
                        ColumnDef::new(AnswersTable::TimeStamp)
                            .timestamp()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(RealAnswersTable::RealAnswers)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RealAnswersTable::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RealAnswersTable::AnswerId)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-answer-id")
                            .from(RealAnswersTable::RealAnswers, RealAnswersTable::AnswerId)
                            .to(AnswersTable::Answers, AnswersTable::Id),
                    )
                    .col(
                        ColumnDef::new(RealAnswersTable::QuestionId)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-question-id-from-real-answers")
                            .from(RealAnswersTable::RealAnswers, RealAnswersTable::QuestionId)
                            .to(
                                FormQuestionsTable::FormQuestions,
                                FormQuestionsTable::QuestionId,
                            ),
                    )
                    .col(ColumnDef::new(RealAnswersTable::Answer).string().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AnswersTable::Answers).to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(RealAnswersTable::RealAnswers)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum AnswersTable {
    Answers,
    Id,
    FormId,
    User,
    Title,
    TimeStamp,
}

#[derive(DeriveIden)]
enum RealAnswersTable {
    RealAnswers,
    Id,
    AnswerId,
    QuestionId,
    Answer,
}
