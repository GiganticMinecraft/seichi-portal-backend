use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UsersTable::Users)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UsersTable::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(UsersTable::Uuid).uuid().not_null())
                    .col(ColumnDef::new(UsersTable::Name).string().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UsersTable::Users).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum UsersTable {
    Users,
    Id,
    Uuid,
    Name,
}
