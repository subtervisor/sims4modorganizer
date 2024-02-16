use inquire::error::InquireResult;
use inquire::{Confirm, InquireError, Select, Text};
use sea_orm::{prelude::*, ActiveValue, Condition, IntoActiveModel, QuerySelect, TransactionTrait};
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
    Quit,
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
        let mut current_state = EditMenuAction::MainMenu;
        let mut previous_menu_state = EditMenuAction::AllModList;
        while current_state != EditMenuAction::Quit {
            match current_state {
                EditMenuAction::MainMenu => {
                    let options: Vec<EditMenuAction> = vec![
                        EditMenuAction::TagList,
                        EditMenuAction::AllModList,
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
                    current_state =
                        Select::new(format!("Mods for tag {}:", tag_name).as_str(), menu_entries)
                            .prompt()
                            .with_interrupted_default(EditMenuAction::TagList)?;
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
                    current_state = Select::new("All Mods:", mod_list_options)
                        .prompt()
                        .with_interrupted_default(EditMenuAction::MainMenu)?;
                    previous_menu_state = EditMenuAction::AllModList;
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
                        .prompt();
                    match new_name_result {
                        Ok(new_name) => {
                            let model_id = mod_model.id;
                            let mut active_model = mod_model.into_active_model();
                            active_model.name = ActiveValue::set(new_name.clone());
                            active_model.updated = ActiveValue::set(chrono::offset::Local::now());
                            active_model.save(&db).await?;
                            current_state = EditMenuAction::EditMod(new_name, model_id);
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
                    .prompt();
                    match confirm_result {
                        Ok(confirm) => {
                            if confirm {
                                let mut active_model = mod_model.clone().into_active_model();
                                db.transaction::<_, (), DbErr>(|txn| {
                                    Box::pin(async move {
                                        mod_tag_relation::Entity::delete_by_id((mod_model.id, tag_id)).exec(txn).await?;
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
                        }
                        Err(inquire_err) => match inquire_err {
                            InquireError::OperationInterrupted
                            | InquireError::OperationCanceled => {
                                current_state = EditMenuAction::EditModTags(mod_model);
                            }
                            _ => return Err(inquire_err.into()),
                        },
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
                        .prompt();
                    match new_tag_result {
                        Ok(new_tag) => {
                            if !existing_tags.contains(&new_tag) {
                                let mut active_model = mod_model.clone().into_active_model();
                                db.transaction::<_, (), DbErr>(|txn| {
                                    Box::pin(async move {
                                        let tag_id =
                                            super::util::get_or_create_tag_id(txn, &new_tag)
                                                .await?;
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
                        }
                        Err(inquire_err) => match inquire_err {
                            InquireError::OperationInterrupted
                            | InquireError::OperationCanceled => {
                                current_state = EditMenuAction::EditModTags(mod_model);
                            }
                            _ => return Err(inquire_err.into()),
                        },
                    }
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
