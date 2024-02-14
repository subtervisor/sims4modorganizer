use sea_orm_migration::prelude::*;

use super::m20220101_000001_create_sims_mods::SimsMod;
use super::m20220101_000002_create_tags::Tag;

#[derive(DeriveMigrationName)]
pub struct ModTagRelationsTableMigration;

#[async_trait::async_trait]
impl MigrationTrait for ModTagRelationsTableMigration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ModTagRelation::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(ModTagRelation::ModId).integer().not_null())
                    .col(ColumnDef::new(ModTagRelation::TagId).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-mod_tag_relations-mod_id")
                            .from(ModTagRelation::Table, ModTagRelation::ModId)
                            .to(SimsMod::Table, SimsMod::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-mod_tag_relations-tag_id")
                            .from(ModTagRelation::Table, ModTagRelation::TagId)
                            .to(Tag::Table, Tag::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .primary_key(
                        Index::create()
                            .col(ModTagRelation::ModId)
                            .col(ModTagRelation::TagId),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ModTagRelation::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ModTagRelation {
    Table,
    ModId,
    TagId,
}
