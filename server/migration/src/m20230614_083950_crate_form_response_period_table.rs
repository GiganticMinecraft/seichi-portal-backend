use sea_orm_migration::prelude::ColumnType::Enum;
use sea_orm_migration::prelude::*;

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
                        ColumnDef::new_with_type(
                            FormResponsePeriodTable::PeriodName.into_iden(),
                            Enum {
                                name: FormResponsePeriodTable::PeriodName.into_iden(),
                                variants: vec![
                                    PeriodNames::StartAt.into_iden(),
                                    PeriodNames::EndAt.into_iden(),
                                ],
                            },
                        )
                        .not_null(),
                    )
                    .col(
                        ColumnDef::new(FormResponsePeriodTable::Time)
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
    PeriodName,
    Time,
}

#[derive(Iden)]
enum PeriodNames {
    StartAt,
    EndAt,
}
