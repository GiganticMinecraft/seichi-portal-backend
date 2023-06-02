use std::borrow::BorrowMut;

use sea_orm_migration::prelude::*;

use crate::{m20220101_000001_create_table::FormMetaDataTable, ColumnType::Enum};

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
                            .to(FormMetaDataTable::FormMetaData, FormMetaDataTable::Id),
                    )
                    .col(
                        ColumnDef::new(FormQuestionsTable::Title)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(FormQuestionsTable::Description).string())
                    .col(
                        ColumnDef::new_with_type(
                            FormQuestionsTable::QuestionType.into_iden(),
                            Enum {
                                name: FormQuestionsTable::QuestionType.into_iden(),
                                variants: vec![
                                    QuestionType::Text.into_iden(),
                                    QuestionType::Multiple.into_iden(),
                                    QuestionType::Single.into_iden(),
                                ],
                            },
                        )
                        .not_null()
                        .borrow_mut(),
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
    QuestionType,
}

#[derive(Iden)]
enum QuestionType {
    Text,
    Single,
    Multiple,
}
