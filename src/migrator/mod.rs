use sea_orm_migration::prelude::*;

mod m20220101_000001_create_sims_mods;
mod m20220101_000002_create_tags;
mod m20220101_000003_create_hashes;
mod m20220101_000004_create_mod_tag_relations;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_sims_mods::SimsModTableMigration),
            Box::new(m20220101_000002_create_tags::ModTagTableMigration),
            Box::new(m20220101_000003_create_hashes::ModHashesTableMigration),
            Box::new(m20220101_000004_create_mod_tag_relations::ModTagRelationsTableMigration),
        ]
    }
}
