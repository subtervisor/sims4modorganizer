//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.14

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sims_mod")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub name: String,
    #[sea_orm(unique)]
    pub directory: String,
    pub source_url: String,
    pub version: String,
    pub updated: DateTimeLocal,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::mod_hash::Entity")]
    ModHash,
    #[sea_orm(has_many = "super::mod_tag_relation::Entity")]
    ModTagRelation,
}

impl Related<super::mod_hash::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ModHash.def()
    }
}

impl Related<super::mod_tag_relation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ModTagRelation.def()
    }
}

impl Related<super::tag::Entity> for Entity {
    fn to() -> RelationDef {
        super::mod_tag_relation::Relation::Tag.def()
    }
    fn via() -> Option<RelationDef> {
        Some(super::mod_tag_relation::Relation::SimsMod.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}
