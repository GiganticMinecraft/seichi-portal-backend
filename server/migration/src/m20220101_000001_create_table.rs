use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FormsTable::Forms)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FormsTable::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(FormsTable::Name).string().not_null())
                    .col(ColumnDef::new(FormsTable::Description).string())
                    .col(ColumnDef::new(FormsTable::CreatedAt).timestamp().not_null())
                    .col(ColumnDef::new(FormsTable::UpdatedAt).timestamp().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(FormsTable::Forms).to_owned())
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
pub enum FormsTable {
    // todo: Formsという名前だと、抽象的すぎて何でもカラムが追加されそうなので具体的な名前にしたい
    Forms,
    Id,
    Name,
    Description,
    CreatedAt,
    UpdatedAt,
}
