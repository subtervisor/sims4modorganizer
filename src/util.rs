use crate::{migrator::Migrator, Result};
use sea_orm_migration::prelude::*;
use std::io::{Error as IOError, ErrorKind as IOErrorKind};
use tracing::{debug, error, info};
use tracing_unwrap::OptionExt;

pub fn get_sims_mod_dir() -> Result<std::path::PathBuf> {
    let sims_mod_dir = dirs::document_dir()
        .expect_or_log("Failed to get Documents directory")
        .join("Electronic Arts")
        .join("The Sims 4")
        .join("Mods");
    if !sims_mod_dir.is_dir() {
        use std::io::*;
        error!("Could not locate Sims 4 mods folder. ");
        Err(Box::new(Error::from(ErrorKind::NotFound)))
    } else {
        Ok(sims_mod_dir)
    }
}

fn get_db_path() -> Result<std::path::PathBuf> {
    let data_dir = dirs::data_dir()
        .expect_or_log("Failed to get user data directory")
        .join("com.familiar.sims4modsorganizer");
    if !data_dir.is_dir() {
        info!("Creating data directory");
        std::fs::create_dir(&data_dir)?;
    }
    Ok(data_dir.join("mods.sqlite"))
}

async fn open_db_internal(create: bool) -> Result<sea_orm::DbConn> {
    debug!("Opening database with create = {}", create);
    let database_path = get_db_path()?;
    debug!("Database path: {}", database_path.display());
    let dbpath = format!(
        "sqlite://{}?mode={}",
        database_path
            .to_str()
            .expect_or_log("Failed to parse path as UTF-8"),
        if create { "rwc" } else { "rw" }
    );
    let connect_options = sea_orm::ConnectOptions::new(dbpath);
    sea_orm::SqlxSqliteConnector::connect(connect_options)
        .await
        .map_err(|e| e.into())
}

pub async fn open_database() -> Result<sea_orm::DbConn> {
    debug!("Opening existing mod database");
    open_db_internal(false).await
}

pub async fn init_database(force: bool) -> Result<()> {
    debug!("Initializing database");
    let database_path = get_db_path()?;
    if database_path.is_file() {
        if force {
            info!("Deleting existing database...");
            tokio::fs::remove_file(database_path).await?;
        } else {
            return Err(IOError::new(IOErrorKind::AlreadyExists, "Database file exists!").into());
        }
    }
    info!("Opening database...");
    let db = open_db_internal(true).await?;
    info!("Initializing...");
    Migrator::refresh(&db).await?;
    Ok(())
}
