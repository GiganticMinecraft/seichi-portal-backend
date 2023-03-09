use sea_orm_migration::prelude::*;

use crate::{
    m20220101_000001_create_table::FormsTable,
    m20221127_173808_create_form_question_type_enum_table::QuestionTypeEnumTable,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FormQuestionsTable::FormQuestions)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FormQuestionsTable::QuestionId)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(FormQuestionsTable::FormId)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-form_key")
                            .from(
                                FormQuestionsTable::FormQuestions,
                                FormQuestionsTable::FormId,
                            )
                            .to(FormsTable::Forms, FormsTable::Id),
                    )
                    .col(
                        ColumnDef::new(FormQuestionsTable::Title)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FormQuestionsTable::Description)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FormQuestionsTable::AnswerType)
                            .string()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-answer_type")
                            .from(
                                FormQuestionsTable::FormQuestions,
                                FormQuestionsTable::AnswerType,
                            )
                            .to(
                                QuestionTypeEnumTable::QuestionTypes,
                                QuestionTypeEnumTable::AnswerType,
                            ),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(FormQuestionsTable::FormQuestions)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
pub enum FormQuestionsTable {
    FormQuestions,
    QuestionId,
    FormId,
    Title,
    Description,
    AnswerType,
}
