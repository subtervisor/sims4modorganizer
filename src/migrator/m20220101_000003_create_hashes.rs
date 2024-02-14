use sea_orm_migration::prelude::*;

use super::m20220101_000001_create_sims_mods::SimsMod;

#[derive(DeriveMigrationName)]
pub struct ModHashesTableMigration;

#[async_trait::async_trait]
impl MigrationTrait for ModHashesTableMigration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ModHash::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ModHash::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ModHash::ModId).integer().not_null())
                    .col(ColumnDef::new(ModHash::File).string().not_null())
                    .col(
                        ColumnDef::new(ModHash::Hash)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-mod_hashes-mod_id")
                            .from(ModHash::Table, ModHash::ModId)
                            .to(SimsMod::Table, SimsMod::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ModHash::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ModHash {
    Table,
    Id,
    ModId,
    File,
    Hash,
}
