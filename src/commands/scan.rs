use crate::entities::{prelude::*, *};
use crate::{commands::util::*, Result as CrateResult};
use colored::*;
use inquire::{Confirm, Text};
use sea_orm::{prelude::*, *};
use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    path::PathBuf,
};
use tracing::{debug, info};
use tracing_unwrap::OptionExt;

async fn ask_delete_mod(db: &DatabaseConnection, missing_mod: &sims_mod::Model) -> CrateResult<()> {
    if Confirm::new(
        format!(
            "Do you want to remove {} from the database?",
            missing_mod.name.bold().red()
        )
        .as_str(),
    )
    .with_default(false)
    .prompt()?
    {
        info!("Deleting {}...", missing_mod.name);
        sims_mod::ActiveModel {
            id: ActiveValue::Set(missing_mod.id),
            ..Default::default()
        }
        .delete(db)
        .await?;
    }
    Ok(())
}

async fn detect_collision(
    txn: &sea_orm::DatabaseTransaction,
    name: &str,
    file: &PathBuf,
    hash: &str,
) -> Result<(), sea_orm::DbErr> {
    let collision = ModHash::find()
        .filter(Condition::any().add(mod_hash::Column::Hash.eq(hash)))
        .one(txn)
        .await?;
    if let Some(collision) = collision {
        eprintln!("{}", "!!! HASH COLLISION DETECTED !!!".bold().red().blink());
        eprintln!("Hash:           {}", collision.hash.bold().yellow());
        eprintln!("Colliding mod:  {}", name.red().bold());
        eprintln!("Colliding file: {}", file.display().to_string().red());
        let existing_mod = collision
            .find_related(SimsMod)
            .one(txn)
            .await?
            .expect_or_log("Failed to find existing mod for collision");
        eprintln!("Existing mod:   {}", existing_mod.name.blue().bold());
        eprintln!("Existing file:  {}", collision.file.blue());
    }
    Ok(())
}

async fn add_mod(db: &DatabaseConnection, path: &PathBuf) -> CrateResult<()> {
    if !Confirm::new(
        format!(
            "Do you want to add {} to the database?",
            path.display().to_string().bold().blue()
        )
        .as_str(),
    )
    .with_default(true)
    .prompt()?
    {
        println!("Ignoring mod...");
        return Ok(());
    }

    info!("Adding {}", path.display());
    let name = Text::new("Name:")
        .with_initial_value(&path.display().to_string())
        .with_validator(inquire::required!())
        .prompt()?;

    let source_url = crate::commands::util::get_source_url(None)?;

    let now = chrono::offset::Local::now();

    let version = Text::new("Version:")
        .with_default(&now.format("%d%m%y").to_string())
        .prompt()?;

    let mut tags: Vec<String> = Text::new("Tags (comma separated):")
        .with_placeholder("Body, Patreon")
        .prompt()?
        .split(",")
        .map(|s| s.trim().to_string())
        .collect::<HashSet<_>>()
        .drain()
        .collect();

    debug!("Fetching file hashes");
    let (_, mut hashes) = crate::commands::util::get_file_hashes(&path)?;

    let path = path
        .to_str()
        .expect_or_log("Failed to convert path to UTF-8")
        .to_string();
    db.transaction::<_, (), DbErr>(|txn| {
        Box::pin(async move {
            info!("Saving {}", name);
            let new_mod = sims_mod::ActiveModel {
                name: ActiveValue::Set(name.clone()),
                directory: ActiveValue::Set(path),
                source_url: ActiveValue::Set(source_url),
                version: ActiveValue::Set(version),
                updated: ActiveValue::Set(now),
                ..Default::default()
            };
            let res = SimsMod::insert(new_mod).exec(txn).await?;
            let last_mod_id = res.last_insert_id;
            debug!("Mod ID: {}", last_mod_id);
            debug!("Adding tags...");
            for mod_tag in tags.drain(..) {
                let tag_id = super::util::get_or_create_tag_id(txn, &mod_tag).await?;
                let new_relationship = mod_tag_relation::ActiveModel {
                    mod_id: ActiveValue::Set(last_mod_id),
                    tag_id: ActiveValue::Set(tag_id),
                };
                debug!("Creating tag relation...");
                ModTagRelation::insert(new_relationship).exec(txn).await?;
            }

            debug!("Adding file hash data");
            for (path, hash) in hashes.drain() {
                debug!("Saving hash for {} ({})", path.display(), hash);
                detect_collision(txn, &name, &path, &hash).await?;
                let new_hash = mod_hash::ActiveModel {
                    mod_id: ActiveValue::Set(last_mod_id),
                    file: ActiveValue::Set(
                        path.to_str()
                            .expect_or_log("Failed to convert path to UTF-8")
                            .to_string(),
                    ),
                    hash: ActiveValue::Set(hash),
                    ..Default::default()
                };
                ModHash::insert(new_hash).exec(txn).await?;
            }

            Ok(())
        })
    })
    .await?;
    debug!("Saved mod!");
    Ok(())
}

async fn update_mod_from_scan(
    db: &DatabaseConnection,
    to_save: sims_mod::Model,
    mut verification: HashMap<PathBuf, String>,
    hash_update: bool,
) -> CrateResult<()> {
    if !hash_update
        && !Confirm::new(
            format!("Do you want to update {}?", to_save.name.yellow().bold()).as_str(),
        )
        .with_default(true)
        .prompt()?
    {
        println!("Leaving existing mod data");
        return Ok(());
    }

    let source_url = to_save.source_url.clone();
    let version = to_save.version.clone();
    let mod_id = to_save.id;
    let name = to_save.name.clone();
    let mut updated_model = to_save.into_active_model();

    updated_model.updated = ActiveValue::Set(chrono::offset::Local::now());
    if !hash_update {
        updated_model.source_url =
            ActiveValue::Set(crate::commands::util::get_source_url(Some(&source_url))?);

        updated_model.version =
            ActiveValue::Set(Text::new("Version:").with_default(&version).prompt()?);
    }

    db.transaction::<_, (), DbErr>(|txn| {
        Box::pin(async move {
            debug!("Updating {}", name);
            SimsMod::update(updated_model).exec(txn).await?;
            debug!("Clearing existing hash data");
            mod_hash::Entity::delete_many()
                .filter(mod_hash::Column::ModId.eq(mod_id))
                .exec(txn)
                .await?;

            debug!("Saving new hash data");
            for (path, hash) in verification.drain() {
                debug!("Saving hash for {} ({})", path.display(), hash);
                detect_collision(txn, &name, &path, &hash).await?;
                let new_hash = mod_hash::ActiveModel {
                    mod_id: ActiveValue::Set(mod_id),
                    file: ActiveValue::Set(
                        path.to_str()
                            .expect_or_log("Failed to convert path to UTF-8")
                            .to_string(),
                    ),
                    hash: ActiveValue::Set(hash),
                    ..Default::default()
                };
                ModHash::insert(new_hash).exec(txn).await?;
            }
            Ok(())
        })
    })
    .await?;

    Ok(())
}

pub async fn scan(verify: bool, fix: bool, hash_update: bool) -> CrateResult<()> {
    debug!("Scanning mods");
    let db = crate::util::open_database().await?;

    let mods = SimsMod::find().all(&db).await?;

    debug!("Reading current mod directory list");
    let mod_dir = crate::util::get_sims_mod_dir()?;
    let mut mod_dir_entries: Vec<_> = mod_dir
        .read_dir()?
        .map(|entry| -> CrateResult<PathBuf> {
            let entry = entry?;
            Ok(entry.path())
        })
        .collect::<std::result::Result<_, _>>()?;

    debug!("Checking for directory changes");
    let mod_dir_subdirs: HashSet<_> = mod_dir_entries
        .drain(..)
        .filter_map(|entry| {
            if entry.is_dir() && entry.file_name() != Some(&OsString::from("mod_data")) {
                Some(PathBuf::from(entry.file_name().unwrap()))
            } else {
                None
            }
        })
        .collect();

    let path_mod_map: std::collections::HashMap<PathBuf, _> = mods
        .iter()
        .map(|sims_mod| (sims_mod.directory.clone().into(), sims_mod))
        .collect();
    let existing_subdirs = HashSet::from_iter(path_mod_map.keys().map(|k| k.to_owned()));

    let mut new_mods: Vec<_> = mod_dir_subdirs.difference(&existing_subdirs).collect();
    let mut missing_mods: Vec<_> = existing_subdirs.difference(&mod_dir_subdirs).collect();
    let mut mods_to_scan: Vec<_> = existing_subdirs.intersection(&mod_dir_subdirs).collect();

    if !new_mods.is_empty() {
        println!(
            "Found {} new mods.",
            new_mods.len().to_string().blue().bold()
        );
        for new_mod in new_mods.drain(..) {
            if fix {
                add_mod(&db, new_mod).await?;
            } else {
                println!("Found mod: {}", new_mod.display().to_string().bold().blue());
            }
        }
    }
    if !missing_mods.is_empty() {
        println!(
            "Found {} missing mods.",
            missing_mods.len().to_string().red().bold()
        );
        for missing_mod in missing_mods.drain(..) {
            let missing_mod_entry = *path_mod_map
                .get(missing_mod)
                .expect_or_log("Failed to get mod from mod map");
            if fix {
                ask_delete_mod(&db, missing_mod_entry).await?;
            } else {
                let name = &path_mod_map
                    .get(missing_mod)
                    .expect_or_log("Failed to get name from mod map")
                    .name;
                println!("Missing mod: {}", name.bold().red());
            }
        }

        info!("Cleaning up tags...");
        super::util::cleanup_tags(&db).await?;
    }

    if (verify || hash_update) && !mods_to_scan.is_empty() {
        println!(
            "Checking {} existing mods.",
            mods_to_scan.len().to_string().bold()
        );
        for to_scan in mods_to_scan.drain(..) {
            let mod_to_scan = *path_mod_map
                .get(to_scan)
                .expect_or_log("Failed to get mod from mod map");
            let mut hashes = super::util::get_hashes_for_mod(&db, mod_to_scan.id).await?;
            let verify_results = crate::commands::util::verify_files(to_scan, &hashes)?;
            if verify_results.verification_passed() {
                println!("Validated mod: {}", mod_to_scan.name.bold().green());
            } else {
                if hash_update {
                    println!("Updating mod:  {}", mod_to_scan.name.bold().yellow());
                } else {
                    println!(
                        "Mod {} validation: {}",
                        "failed".red(),
                        mod_to_scan.name.bold().green()
                    );
                }
                if fix || hash_update {
                    for missing in verify_results.missing_files.iter() {
                        hashes.remove(missing);
                    }

                    for (file, hash) in verify_results.changed_files.iter() {
                        hashes.insert(file.clone(), hash.clone());
                    }

                    for (file, hash) in verify_results.new_files.iter() {
                        hashes.insert(file.clone(), hash.clone());
                    }
                    update_mod_from_scan(&db, mod_to_scan.clone(), hashes, hash_update).await?;
                }
            }
        }
    }
    Ok(())
}
