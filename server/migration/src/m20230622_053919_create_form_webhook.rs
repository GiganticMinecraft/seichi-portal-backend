use crate::m20220101_000001_create_table::FormMetaDataTable;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FormWebhookTable::FormWebhooks)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FormWebhookTable::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(FormWebhookTable::FormId)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-form_key_form_webhooks")
                            .from(FormWebhookTable::FormWebhooks, FormWebhookTable::FormId)
                            .to(FormMetaDataTable::FormMetaData, FormMetaDataTable::Id),
                    )
                    .col(ColumnDef::new(FormWebhookTable::Url).string().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(FormWebhookTable::FormWebhooks)
                    .to_owned(),
            )
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum FormWebhookTable {
    FormWebhooks,
    Id,
    FormId,
    Url,
}
