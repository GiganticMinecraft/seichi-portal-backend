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
                    .table(FormResponsePeriodTable::ResponsePeriod)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FormResponsePeriodTable::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(FormResponsePeriodTable::FormId)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-form_key_from_response_period")
                            .from(
                                FormResponsePeriodTable::ResponsePeriod,
                                FormResponsePeriodTable::FormId,
                            )
                            .to(FormMetaDataTable::FormMetaData, FormMetaDataTable::Id),
                    )
                    .col(
                        ColumnDef::new(FormResponsePeriodTable::StartAt)
                            .date_time()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FormResponsePeriodTable::EndAt)
                            .date_time()
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
                    .table(FormResponsePeriodTable::ResponsePeriod)
                    .to_owned(),
            )
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum FormResponsePeriodTable {
    ResponsePeriod,
    Id,
    FormId,
    StartAt,
    EndAt,
}
