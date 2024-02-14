use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct SimsModTableMigration;

#[async_trait::async_trait]
impl MigrationTrait for SimsModTableMigration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SimsMod::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SimsMod::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SimsMod::Name)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(SimsMod::Directory)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(SimsMod::SourceUrl).string().not_null())
                    .col(ColumnDef::new(SimsMod::Version).string().not_null())
                    .col(ColumnDef::new(SimsMod::Updated).timestamp().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SimsMod::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum SimsMod {
    Table,
    Id,
    Name,
    Directory,
    SourceUrl,
    Version,
    Updated,
}
