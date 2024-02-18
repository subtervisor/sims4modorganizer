use crate::entities::{prelude::*, *};
use crate::Result as CrateResult;
use inquire::autocompletion::Replacement;
use inquire::error::InquireResult;
use inquire::CustomUserError;
use itertools::Itertools;
use radix_trie::TrieCommon;
use sea_orm::{prelude::*, *};
use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    path::PathBuf,
};
use tracing::debug;
use tracing_unwrap::OptionExt;
use xxhash_rust::xxh3::xxh3_64;

pub struct VerificationValues {
    pub new_files: HashMap<PathBuf, String>,
    pub missing_files: Vec<PathBuf>,
    pub matching_files: Vec<PathBuf>,
    pub changed_files: HashMap<PathBuf, String>,
}

pub trait VerificationPassed {
    fn verification_passed(&self) -> bool;
}

impl VerificationPassed for VerificationValues {
    fn verification_passed(&self) -> bool {
        self.new_files.is_empty() && self.missing_files.is_empty() && self.changed_files.is_empty()
    }
}

pub fn get_file_hashes(
    mod_path: &PathBuf,
) -> CrateResult<(HashSet<PathBuf>, HashMap<PathBuf, String>)> {
    let final_mod_path = crate::util::get_sims_mod_dir()?.join(mod_path);
    debug!("Scanning files in {}", mod_path.display());
    let mut files: Vec<PathBuf> = final_mod_path
        .read_dir()?
        .map(|entry| -> CrateResult<PathBuf> {
            let entry = entry?;
            Ok(entry.path())
        })
        .collect::<Result<_, _>>()?;
    let current_packages: HashSet<_> = files
        .drain(..)
        .filter(|path| {
            if path.is_file()
                && (path.extension() == Some(&OsString::from("package"))
                    || path.extension() == Some(&OsString::from("ts4script")))
            {
                true
            } else {
                false
            }
        })
        .map(|path| PathBuf::from(path.file_name().expect_or_log("Path is invalid!")))
        .collect();

    debug!("Gathering checksums for {} files", current_packages.len());

    let hashes = current_packages
        .iter()
        .map(|file_path| -> CrateResult<_> {
            debug!("Generating checksum for {}", file_path.display());
            let mod_file_path = final_mod_path.join(file_path);
            let mod_file_data = std::fs::read(mod_file_path)?;
            Ok((
                file_path.clone(),
                format!("{:#10x}", xxh3_64(&mod_file_data)),
            ))
        })
        .collect::<Result<_, _>>()?;
    Ok((current_packages, hashes))
}

pub fn verify_files(
    mod_path: &PathBuf,
    hashes: &HashMap<PathBuf, String>,
) -> CrateResult<VerificationValues> {
    debug!("Verifying mod_path {}", mod_path.display());

    let (current_packages, package_hashes) = get_file_hashes(mod_path)?;

    debug!("Sorting verification statuses");
    let db_file_list: HashSet<_> = hashes.keys().map(|k| k.clone()).collect();

    let missing_files: Vec<PathBuf> = db_file_list
        .difference(&current_packages)
        .map(|k| k.clone())
        .collect();
    let new_files: HashSet<_> = current_packages.difference(&db_file_list).collect();
    let new_files: HashMap<PathBuf, String> = package_hashes
        .iter()
        .filter_map(|(k, v)| {
            if new_files.contains(k) {
                Some((k.clone(), v.clone()))
            } else {
                None
            }
        })
        .collect();

    let common_files: Vec<_> = db_file_list.intersection(&current_packages).collect();
    let matching_files: Vec<PathBuf> = common_files
        .iter()
        .filter_map(|file| -> Option<PathBuf> {
            if package_hashes.get(*file) == hashes.get(*file) {
                Some((*file).clone())
            } else {
                None
            }
        })
        .collect();
    let changed_files: HashMap<PathBuf, String> = common_files
        .iter()
        .filter_map(|file| -> Option<(PathBuf, String)> {
            if matching_files.contains(file) {
                None
            } else {
                Some(((*file).clone(), package_hashes.get(*file).unwrap().clone()))
            }
        })
        .collect();
    Ok(VerificationValues {
        new_files,
        missing_files,
        matching_files,
        changed_files,
    })
}

#[derive(Clone)]
pub struct URLValidator {}

impl URLValidator {
    /// Create a new instance of this validator with given error message.
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for URLValidator {
    /// Create a new instance of this validator with the default error message
    /// `A response is required`.
    fn default() -> Self {
        Self::new()
    }
}

impl inquire::validator::StringValidator for URLValidator {
    fn validate(
        &self,
        input: &str,
    ) -> Result<inquire::validator::Validation, inquire::CustomUserError> {
        match url::Url::parse(input) {
            Ok(_) => Ok(inquire::validator::Validation::Valid),
            Err(e) => Ok(inquire::validator::Validation::Invalid(
                format!("Failed to validate URL: {}", e).as_str().into(),
            )),
        }
    }
}
pub fn get_source_url_raw(current: Option<&str>) -> InquireResult<String> {
    let prompt = inquire::Text::new("Source URL:").with_placeholder("https://myshuno.web/mod");

    if let Some(current) = current {
        prompt.with_default(current).with_initial_value(current)
    } else {
        prompt
    }
    .with_validator(inquire::required!())
    .with_validator(URLValidator::default())
    .prompt()
}

pub fn get_source_url(current: Option<&str>) -> CrateResult<String> {
    get_source_url_raw(current).map_err(|e| e.into())
}

pub async fn get_or_create_tag_id<C>(db: &C, tag: &str) -> Result<i32, DbErr>
where
    C: sea_orm::ConnectionTrait,
{
    let tag_id = Tag::find()
        .filter(tag::Column::Tag.eq(tag))
        .one(db)
        .await?
        .map(|t| t.id);

    if let Some(tag_id) = tag_id {
        debug!("Existing tag ID for {}: {}", tag, tag_id);
        Ok(tag_id)
    } else {
        debug!("Adding tag: {}", tag);
        let new_tag = tag::ActiveModel {
            tag: ActiveValue::Set(tag.to_string()),
            ..Default::default()
        };

        let res = Tag::insert(new_tag).exec(db).await?;
        debug!("New tag ID: {}", res.last_insert_id);
        Ok(res.last_insert_id)
    }
}

pub async fn get_mods_for_tags<C>(
    db: &C,
    mut tags: Vec<String>,
) -> CrateResult<Vec<crate::entities::sims_mod::Model>>
where
    C: sea_orm::ConnectionTrait,
{
    debug!("Fetching ids for tags {:?}", tags);
    let tag_id_cond = Tag::find()
        .filter(
            tags.drain(..)
                .fold(Condition::any(), |c, t| c.add(tag::Column::Tag.eq(t))),
        )
        .all(db)
        .await?
        .drain(..)
        .fold(Condition::any(), |c, t| {
            c.add(mod_tag_relation::Column::TagId.eq(t.id))
        });
    debug!("Fetching tag relations");
    let tag_relations_cond = ModTagRelation::find()
        .filter(tag_id_cond)
        .all(db)
        .await?
        .drain(..)
        .fold(Condition::any(), |c, r| {
            c.add(sims_mod::Column::Id.eq(r.mod_id))
        });
    Ok(SimsMod::find().filter(tag_relations_cond).all(db).await?)
}

pub async fn get_tags_for_mod<C>(db: &C, mod_id: i32) -> CrateResult<Vec<String>>
where
    C: sea_orm::ConnectionTrait,
{
    let tag_relation_cond = ModTagRelation::find()
        .filter(Condition::any().add(mod_tag_relation::Column::ModId.eq(mod_id)))
        .all(db)
        .await?
        .drain(..)
        .fold(Condition::any(), |c, r| c.add(tag::Column::Id.eq(r.tag_id)));
    Ok(Tag::find()
        .filter(tag_relation_cond)
        .all(db)
        .await?
        .drain(..)
        .map(|t| t.tag)
        .collect())
}

pub async fn get_hashes_for_mod<C>(db: &C, mod_id: i32) -> CrateResult<HashMap<PathBuf, String>>
where
    C: sea_orm::ConnectionTrait,
{
    Ok(ModHash::find()
        .filter(Condition::any().add(mod_hash::Column::ModId.eq(mod_id)))
        .all(db)
        .await?
        .drain(..)
        .map(|he| (he.file.into(), he.hash))
        .collect())
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
enum QueryAs {
    TagId,
}

pub async fn cleanup_tags<C>(db: &C) -> Result<(), DbErr>
where
    C: sea_orm::ConnectionTrait,
{
    debug!("Cleaning up unused tags");
    let unused_tags_cond = ModTagRelation::find()
        .select_only()
        .column_as(mod_tag_relation::Column::TagId, QueryAs::TagId)
        .into_values::<i32, QueryAs>()
        .all(db)
        .await?
        .drain(..)
        .unique()
        .fold::<Condition, _>(Condition::all(), |c, i| c.add(tag::Column::Id.ne(i)));
    let result = Tag::delete_many().filter(unused_tags_cond).exec(db).await?;
    debug!("Deleted {} tags", result.rows_affected);
    Ok(())
}

#[derive(Clone)]
pub struct TagAutoComplete {
    pub tag_trie: radix_trie::Trie<String, ()>,
}

impl TagAutoComplete {
    pub async fn create<C>(db: &C) -> Result<TagAutoComplete, DbErr>
    where
        C: sea_orm::ConnectionTrait,
    {
        return Ok(TagAutoComplete {
            tag_trie: Tag::find()
                .all(db)
                .await?
                .drain(..)
                .map(|m| (m.tag, ()))
                .collect::<radix_trie::Trie<String, ()>>(),
        });
    }

    pub async fn create_with_exclusions<C>(
        db: &C,
        exclusions: &Vec<String>,
    ) -> Result<TagAutoComplete, DbErr>
    where
        C: sea_orm::ConnectionTrait,
    {
        return Ok(TagAutoComplete {
            tag_trie: Tag::find()
                .filter(
                    exclusions
                        .iter()
                        .fold(Condition::all(), |c, e| c.add(tag::Column::Tag.eq(e))),
                )
                .all(db)
                .await?
                .drain(..)
                .map(|m| (m.tag, ()))
                .collect::<radix_trie::Trie<String, ()>>(),
        });
    }
}

impl inquire::Autocomplete for TagAutoComplete {
    // Required methods
    fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, CustomUserError> {
        if let Some(suggestions) = self.tag_trie.subtrie(input) {
            return Ok(suggestions.keys().map(|k| k.clone()).collect());
        }
        Ok(vec![])
    }

    fn get_completion(
        &mut self,
        input: &str,
        highlighted_suggestion: Option<String>,
    ) -> Result<Replacement, CustomUserError> {
        if highlighted_suggestion.is_some() {
            Ok(highlighted_suggestion)
        } else {
            let suggestions = self.get_suggestions(input)?;
            if !suggestions.is_empty() {
                Ok(suggestions.first().map(|s| s.clone()))
            } else {
                Ok(None)
            }
        }
    }
}
