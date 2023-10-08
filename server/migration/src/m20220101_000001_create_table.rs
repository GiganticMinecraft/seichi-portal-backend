use crate::m20231008_135425_create_user_table::UsersTable;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FormMetaDataTable::FormMetaData)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FormMetaDataTable::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(FormMetaDataTable::Title).string().not_null())
                    .col(ColumnDef::new(FormMetaDataTable::Description).string())
                    .col(
                        ColumnDef::new(FormMetaDataTable::CreatedAt)
                            .timestamp()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .col(
                        ColumnDef::new(FormMetaDataTable::CreatedBy)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-created-user-id")
                            .from(
                                FormMetaDataTable::FormMetaData,
                                FormMetaDataTable::CreatedBy,
                            )
                            .to(UsersTable::Users, UsersTable::Id),
                    )
                    .col(
                        ColumnDef::new(FormMetaDataTable::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .col(
                        ColumnDef::new(FormMetaDataTable::UpdatedBy)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-updated-user-id")
                            .from(
                                FormMetaDataTable::FormMetaData,
                                FormMetaDataTable::UpdatedBy,
                            )
                            .to(UsersTable::Users, UsersTable::Id),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(FormMetaDataTable::FormMetaData)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
pub enum FormMetaDataTable {
    FormMetaData,
    Id,
    Title,
    Description,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}
