//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.1

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "answers")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(column_type = "Binary(BlobSize::Blob(Some(16)))")]
    pub user: Vec<u8>,
    pub time_stamp: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::real_answers::Entity")]
    RealAnswers,
}

impl Related<super::real_answers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RealAnswers.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}