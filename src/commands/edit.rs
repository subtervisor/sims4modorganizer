use std::collections::HashSet;
use std::fmt::Debug;

use inquire::error::InquireResult;
use inquire::{Confirm, InquireError, MultiSelect, Select, Text};
use sea_orm::{prelude::*, ActiveValue, Condition, IntoActiveModel, QuerySelect, TransactionTrait};
use tracing::debug;
use tracing_unwrap::OptionExt;

use crate::entities::sims_mod::Model as SimsModModel;

use crate::entities::{prelude::*, *};

#[derive(Debug, Clone, PartialEq)]
enum EditMenuAction {
    MainMenu,
    TagList,
    TagModList(String, i32),
    AllModList,
    EditMod(String, i32),
    EditModName(SimsModModel),
    EditModSource(SimsModModel),
    EditModTags(SimsModModel),
    AddTag(SimsModModel),
    DeleteTag(SimsModModel, String, i32),
    BulkTag,
    ScanNoVerify,
    ScanVerify,
    Quit,
}

struct BulkTagSelection<'a> {
    name: &'a str,
    id: i32,
}

impl std::fmt::Display for BulkTagSelection<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.id)
    }
}

impl std::fmt::Display for EditMenuAction {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            EditMenuAction::MainMenu => write!(f, "Back to main menu"),
            EditMenuAction::TagList => write!(f, "Mods by tag"),
            EditMenuAction::TagModList(tag_name, _) => write!(f, "{}", tag_name),
            EditMenuAction::AllModList => write!(f, "All mods"),
            EditMenuAction::EditMod(mod_name, mod_id) => {
                write!(f, "{} ({})", mod_name, mod_id)
            }
            EditMenuAction::EditModName(mod_model) => write!(f, "Name: {}", mod_model.name),
            EditMenuAction::EditModSource(mod_model) => {
                write!(f, "Source: {}", mod_model.source_url)
            }
            EditMenuAction::EditModTags(mod_model) => write!(f, "Edit tags for {}", mod_model.name),
            EditMenuAction::AddTag(_) => write!(f, "Add tag"),
            EditMenuAction::DeleteTag(_, tag_name, _) => write!(f, "Delete tag {}", tag_name),
            EditMenuAction::BulkTag => write!(f, "Bulk tag mods"),
            EditMenuAction::ScanNoVerify => write!(f, "Scan for new/deleted mods"),
            EditMenuAction::ScanVerify => write!(f, "Scan for new/deleted/updated mods"),
            EditMenuAction::Quit => write!(f, "Quit"),
        }
    }
}

trait InterruptedDefault<T> {
    fn with_interrupted_default(self, d: T) -> InquireResult<T>;
}

impl<T> InterruptedDefault<T> for InquireResult<T> {
    fn with_interrupted_default(self, d: T) -> InquireResult<T> {
        match &self {
            Ok(_) => self,
            Err(ie) => match ie {
                InquireError::OperationCanceled | InquireError::OperationInterrupted => Ok(d),
                _ => self,
            },
        }
    }
}

#[derive(Debug)]
enum DBOrInquireError {
    DB(DbErr),
    Inquire(InquireError),
}

impl std::fmt::Display for DBOrInquireError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::DB(e) => std::fmt::Display::fmt(e, f),
            Self::Inquire(e) => std::fmt::Display::fmt(e, f),
        }
    }
}

impl From<DbErr> for DBOrInquireError {
    fn from(error: DbErr) -> Self {
        DBOrInquireError::DB(error)
    }
}

impl From<InquireError> for DBOrInquireError {
    fn from(error: InquireError) -> Self {
        DBOrInquireError::Inquire(error)
    }
}

impl std::error::Error for DBOrInquireError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

pub async fn edit(
    interactive: bool,
    id: Option<i32>,
    name: Option<String>,
    source_url: Option<String>,
    tags: Option<Vec<String>>,
    version: Option<String>,
) -> crate::Result<()> {
    let db = crate::util::open_database().await?;
    if interactive {
        if id.is_some()
            || name.is_some()
            || source_url.is_some()
            || tags.is_some()
            || version.is_some()
        {
            eprintln!("Interactive mode is not compatible with other arguments");
            return Err(std::io::Error::from(std::io::ErrorKind::Unsupported).into());
        }
        debug!("Entering interactive menu mode");
        let mut current_state = EditMenuAction::MainMenu;
        let mut previous_menu_state = EditMenuAction::AllModList;
        while current_state != EditMenuAction::Quit {
            debug!(
                "Menu state: current = {}, previous = {}",
                current_state, previous_menu_state
            );
            match current_state {
                EditMenuAction::MainMenu => {
                    let options: Vec<EditMenuAction> = vec![
                        EditMenuAction::TagList,
                        EditMenuAction::AllModList,
                        EditMenuAction::BulkTag,
                        EditMenuAction::ScanNoVerify,
                        EditMenuAction::ScanVerify,
                        EditMenuAction::Quit,
                    ];
                    current_state = Select::new("Main Menu:", options)
                        .prompt()
                        .with_interrupted_default(EditMenuAction::Quit)?;
                }
                EditMenuAction::TagList => {
                    let menu_entries: Vec<EditMenuAction> = Tag::find()
                        .select_only()
                        .column(tag::Column::Tag)
                        .column(tag::Column::Id)
                        .into_tuple()
                        .all(&db)
                        .await?
                        .drain(..)
                        .map(|(tag_name, tag_id)| EditMenuAction::TagModList(tag_name, tag_id))
                        .collect();
                    if menu_entries.is_empty() {
                        eprintln!("No tags found!");
                        current_state = EditMenuAction::MainMenu;
                    } else {
                        current_state = Select::new("Mods by tag:", menu_entries)
                            .prompt()
                            .with_interrupted_default(EditMenuAction::MainMenu)?;
                    }
                }
                EditMenuAction::TagModList(tag_name, tag_id) => {
                    let (_, mut tag_mods) = Tag::find_by_id(tag_id)
                        .find_with_related(SimsMod)
                        .all(&db)
                        .await?
                        .pop()
                        .expect_or_log(format!("Failed to get mods for tag {}", tag_name).as_str());
                    let menu_entries: Vec<_> = tag_mods
                        .drain(..)
                        .map(|m| EditMenuAction::EditMod(m.name, m.id))
                        .collect();
                    if menu_entries.is_empty() {
                        eprintln!("There are no mods tagged {}!", tag_name);
                        current_state = EditMenuAction::TagList;
                    } else {
                        current_state = Select::new(
                            format!("Mods for tag {}:", tag_name).as_str(),
                            menu_entries,
                        )
                        .prompt()
                        .with_interrupted_default(EditMenuAction::TagList)?;
                    }
                }
                EditMenuAction::AllModList => {
                    let mod_list_options: Vec<EditMenuAction> = SimsMod::find()
                        .select_only()
                        .column(sims_mod::Column::Name)
                        .column(sims_mod::Column::Id)
                        .into_tuple()
                        .all(&db)
                        .await?
                        .drain(..)
                        .map(|(mod_name, mod_id)| EditMenuAction::EditMod(mod_name, mod_id))
                        .collect();
                    if mod_list_options.is_empty() {
                        eprintln!("There are no mods in the database!");
                        current_state = EditMenuAction::MainMenu;
                    } else {
                        current_state = Select::new("All Mods:", mod_list_options)
                            .prompt()
                            .with_interrupted_default(EditMenuAction::MainMenu)?;
                        previous_menu_state = EditMenuAction::AllModList;
                    }
                }
                EditMenuAction::EditMod(mod_name, mod_id) => {
                    let mod_model = SimsMod::find_by_id(mod_id)
                        .one(&db)
                        .await?
                        .expect_or_log("Failed to load mod by ID!");
                    let options: Vec<EditMenuAction> = vec![
                        EditMenuAction::EditModName(mod_model.clone()),
                        EditMenuAction::EditModSource(mod_model.clone()),
                        EditMenuAction::EditModTags(mod_model.clone()),
                        previous_menu_state.clone(),
                    ];
                    current_state =
                        Select::new(format!("Edit mod {}:", mod_name).as_str(), options)
                            .prompt()
                            .with_interrupted_default(previous_menu_state.clone())?;
                }
                EditMenuAction::EditModName(mod_model) => {
                    let new_name_result = Text::new("Mod name:")
                        .with_initial_value(&mod_model.name)
                        .with_default(&mod_model.name)
                        .with_validator(inquire::required!())
                        .prompt_skippable()?;
                    if let Some(new_name) = new_name_result {
                        let model_id = mod_model.id;
                        let mut active_model = mod_model.into_active_model();
                        active_model.name = ActiveValue::set(new_name.clone());
                        active_model.updated = ActiveValue::set(chrono::offset::Local::now());
                        active_model.save(&db).await?;
                        current_state = EditMenuAction::EditMod(new_name, model_id);
                    } else {
                        current_state = EditMenuAction::EditMod(mod_model.name, mod_model.id)
                    }
                }
                EditMenuAction::EditModSource(mod_model) => {
                    let new_source_result =
                        super::util::get_source_url_raw(Some(&mod_model.source_url));
                    match new_source_result {
                        Ok(new_source) => {
                            let model_id = mod_model.id;
                            let model_name = mod_model.name.clone();
                            let mut active_model = mod_model.into_active_model();
                            active_model.source_url = ActiveValue::set(new_source);
                            active_model.updated = ActiveValue::set(chrono::offset::Local::now());
                            active_model.save(&db).await?;
                            current_state = EditMenuAction::EditMod(model_name, model_id);
                        }
                        Err(inquire_err) => match inquire_err {
                            InquireError::OperationInterrupted
                            | InquireError::OperationCanceled => {
                                current_state =
                                    EditMenuAction::EditMod(mod_model.name, mod_model.id);
                            }
                            _ => return Err(inquire_err.into()),
                        },
                    }
                }
                EditMenuAction::EditModTags(mod_model) => {
                    let mut tag_options: Vec<_> = mod_model
                        .find_related(Tag)
                        .all(&db)
                        .await?
                        .drain(..)
                        .map(|m| EditMenuAction::DeleteTag(mod_model.clone(), m.tag, m.id))
                        .collect();
                    tag_options.push(EditMenuAction::AddTag(mod_model.clone()));
                    let return_option =
                        EditMenuAction::EditMod(mod_model.name.clone(), mod_model.id);
                    current_state = Select::new(
                        format!("Edit tags for {}:", mod_model.name).as_str(),
                        tag_options,
                    )
                    .prompt()
                    .with_interrupted_default(return_option)?;
                }
                EditMenuAction::DeleteTag(mod_model, tag_name, tag_id) => {
                    let mod_name = mod_model.name.clone();
                    let confirm_result = Confirm::new(
                        format!("Remove tag '{}' from {}?", tag_name, mod_name).as_str(),
                    )
                    .with_default(false)
                    .prompt_skippable()?;
                    if let Some(confirm) = confirm_result {
                        if confirm {
                            let mut active_model = mod_model.clone().into_active_model();
                            db.transaction::<_, (), DbErr>(|txn| {
                                Box::pin(async move {
                                    mod_tag_relation::Entity::delete_by_id((mod_model.id, tag_id))
                                        .exec(txn)
                                        .await?;
                                    active_model.updated =
                                        ActiveValue::set(chrono::offset::Local::now());
                                    active_model.save(txn).await?;
                                    super::util::cleanup_tags(txn).await?;
                                    Ok(())
                                })
                            })
                            .await?;
                        }
                        current_state = EditMenuAction::EditModTags(mod_model);
                    } else {
                        current_state = EditMenuAction::EditModTags(mod_model);
                    }
                }
                EditMenuAction::AddTag(mod_model) => {
                    let existing_tags = mod_model
                        .find_related(Tag)
                        .all(&db)
                        .await?
                        .drain(..)
                        .map(|m| m.tag)
                        .collect::<Vec<_>>();
                    let new_tag_result = Text::new("Enter tag:")
                        .with_validator(inquire::required!())
                        .with_autocomplete(
                            super::util::TagAutoComplete::create_with_exclusions(
                                &db,
                                &existing_tags,
                            )
                            .await?,
                        )
                        .prompt_skippable()?;
                    if let Some(new_tag) = new_tag_result {
                        if !existing_tags.contains(&new_tag) {
                            let mut active_model = mod_model.clone().into_active_model();
                            db.transaction::<_, (), DbErr>(|txn| {
                                Box::pin(async move {
                                    let tag_id =
                                        super::util::get_or_create_tag_id(txn, &new_tag).await?;
                                    let relation_model = mod_tag_relation::ActiveModel {
                                        mod_id: ActiveValue::set(mod_model.id),
                                        tag_id: ActiveValue::set(tag_id),
                                    };
                                    ModTagRelation::insert(relation_model).exec(txn).await?;
                                    active_model.updated =
                                        ActiveValue::set(chrono::offset::Local::now());
                                    active_model.save(txn).await?;
                                    Ok(())
                                })
                            })
                            .await?;
                        }
                        current_state = EditMenuAction::EditModTags(mod_model);
                    } else {
                        current_state = EditMenuAction::EditModTags(mod_model);
                    }
                }
                EditMenuAction::BulkTag => {
                    if SimsMod::find().limit(1).one(&db).await?.is_none() {
                        eprintln!("No mods in database to tag!");
                        current_state = EditMenuAction::MainMenu;
                        continue;
                    }
                    let tag_result = Text::new("Enter a tag:")
                        .with_validator(inquire::required!())
                        .with_autocomplete(super::util::TagAutoComplete::create(&db).await?)
                        .prompt_skippable()?;
                    if let Some(bulk_tag) = tag_result {
                        db.transaction::<_, (), DBOrInquireError>(|txn| {
                            Box::pin(async move {
                                let tag_id =
                                    super::util::get_or_create_tag_id(txn, &bulk_tag).await?;
                                let tag_mods = Tag::find_by_id(tag_id)
                                    .find_with_related(SimsMod)
                                    .all(txn)
                                    .await?
                                    .pop()
                                    .map(|(_, mods)| mods)
                                    .expect_or_log(
                                        format!("Failed to get existing mods for tag {}", bulk_tag)
                                            .as_str(),
                                    )
                                    .drain(..)
                                    .map(|m| m.id)
                                    .collect::<HashSet<_>>();

                                let all_mods = SimsMod::find().all(txn).await?;

                                let mod_options = all_mods
                                    .iter()
                                    .map(|m| BulkTagSelection {
                                        name: &m.name,
                                        id: m.id,
                                    })
                                    .collect::<Vec<_>>();

                                let tagged_mod_indexes = mod_options
                                    .iter()
                                    .enumerate()
                                    .filter_map(|(idx, m)| {
                                        if tag_mods.contains(&m.id) {
                                            Some(idx)
                                        } else {
                                            None
                                        }
                                    })
                                    .collect::<Vec<usize>>();
                                let current_tagged_mod_set_indexes =
                                    HashSet::<&usize>::from_iter(tagged_mod_indexes.iter());

                                let formatter: inquire::formatter::MultiOptionFormatter<
                                    '_,
                                    BulkTagSelection,
                                > = &|a| {
                                    let new_tagged_mod_set =
                                        HashSet::<&usize>::from_iter(a.iter().map(|o| &o.index));
                                    let added_tags = new_tagged_mod_set
                                        .difference(&current_tagged_mod_set_indexes)
                                        .count();
                                    let removed_tags = current_tagged_mod_set_indexes
                                        .difference(&new_tagged_mod_set)
                                        .count();
                                    format!(
                                        "Tagged {} mods, untagged {} mods",
                                        added_tags, removed_tags
                                    )
                                };

                                let selection_result = MultiSelect::new(
                                    format!("Select mods to tag {}:", bulk_tag).as_str(),
                                    mod_options,
                                )
                                .with_formatter(formatter)
                                .with_default(&tagged_mod_indexes)
                                .prompt_skippable()?;
                                if let Some(selection) = selection_result {
                                    let selected_mod_ids =
                                        selection.iter().map(|bts| bts.id).collect::<HashSet<_>>();
                                    for mid in selected_mod_ids.difference(&tag_mods) {
                                        let new_model = mod_tag_relation::ActiveModel {
                                            mod_id: ActiveValue::set(*mid),
                                            tag_id: ActiveValue::set(tag_id),
                                        };
                                        ModTagRelation::insert(new_model).exec(txn).await?;
                                    }
                                    ModTagRelation::delete_many()
                                        .filter(
                                            Condition::all()
                                                .add(mod_tag_relation::Column::TagId.eq(tag_id))
                                                .add(tag_mods.difference(&selected_mod_ids).fold(
                                                    Condition::any(),
                                                    |c, i| {
                                                        c.add(
                                                            mod_tag_relation::Column::ModId.eq(*i),
                                                        )
                                                    },
                                                )),
                                        )
                                        .exec(txn)
                                        .await?;
                                }
                                super::util::cleanup_tags(txn).await?;
                                Ok(())
                            })
                        })
                        .await?;
                    } else {
                        current_state = EditMenuAction::MainMenu;
                    }
                }
                EditMenuAction::ScanNoVerify => {
                    super::scan(Some(db.clone()), false, true, false).await?;
                    current_state = EditMenuAction::MainMenu;
                }
                EditMenuAction::ScanVerify => {
                    super::scan(Some(db.clone()), true, true, false).await?;
                    current_state = EditMenuAction::MainMenu;
                }
                EditMenuAction::Quit => {
                    println!("Exiting...");
                }
            }
        }
    } else {
        // id: Option<i32>, name: Option<String>, source_url: Option<String>, tags: Option<Vec<String>>, version: Option<String>
        let id = id.expect_or_log("ID not present in non-interactive mode!");
        let sims_mod = SimsMod::find_by_id(id).one(&db).await?;
        if let Some(sims_mod) = sims_mod {
            db.transaction::<_, (), DbErr>(|txn| {
                Box::pin(async move {
                    let mut active_model = sims_mod.clone().into_active_model();
                    if let Some(name) = name {
                        active_model.name = ActiveValue::set(name);
                    }
                    if let Some(source_url) = source_url {
                        active_model.source_url = ActiveValue::set(source_url);
                    }
                    if let Some(version) = version {
                        active_model.version = ActiveValue::set(version);
                    }
                    if let Some(mut tags) = tags {
                        ModTagRelation::delete_many()
                            .filter(Condition::any().add(mod_tag_relation::Column::ModId.eq(id)))
                            .exec(txn)
                            .await?;
                        for tag in tags.drain(..) {
                            let new_relation = mod_tag_relation::ActiveModel {
                                mod_id: ActiveValue::Set(id),
                                tag_id: ActiveValue::Set(
                                    super::util::get_or_create_tag_id(txn, &tag).await?,
                                ),
                            };
                            ModTagRelation::insert(new_relation).exec(txn).await?;
                        }
                    }
                    active_model.updated = ActiveValue::set(chrono::offset::Local::now());
                    active_model.save(txn).await?;
                    super::util::cleanup_tags(txn).await?;
                    Ok(())
                })
            })
            .await?;
        } else {
            eprintln!("No mod with mod ID {} found!", id);
        }
    }

    Ok(())
}
