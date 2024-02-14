use std::path::PathBuf;

use colored::Colorize;
use sea_orm::prelude::*;

use crate::{commands::util::VerificationPassed, entities::prelude::*};

use super::util;
use tracing_unwrap::OptionExt;

pub async fn list(tags: Option<Vec<String>>, verify: bool, details: bool) -> crate::Result<()> {
    let db = crate::util::open_database().await?;

    let mods = if let Some(tags) = tags {
        util::get_mods_for_tags(&db, tags).await?
    } else {
        SimsMod::find().all(&db).await?
    };

    for sims_mod in mods.iter() {
        let mod_path: PathBuf = sims_mod.directory.clone().into();
        let tags = if details {
            Some(util::get_tags_for_mod(&db, sims_mod.id).await?)
        } else {
            None
        };
        let hashes = if details || verify {
            Some(util::get_hashes_for_mod(&db, sims_mod.id).await?)
        } else {
            None
        };

        let verification_result = if verify {
            if let Some(hashes) = &hashes {
                Some(util::verify_files(&mod_path, hashes)?)
            } else {
                None
            }
        } else {
            None
        };

        let title_corner = boxy::Char::upper_left(boxy::Weight::Thick);
        let title_side_h = boxy::Char::horizontal(boxy::Weight::Thick).to_string();
        let title_side_v = boxy::Char::vertical(boxy::Weight::Thick);
        let title_branch = boxy::Char::right_tee(boxy::Weight::Thick).down(boxy::Weight::Normal);
        let left_branch_more = boxy::Char::right_tee(boxy::Weight::Normal);
        let left_branch_more_str = boxy::Char::right_tee(boxy::Weight::Normal).to_string();
        let left_branch_done = boxy::Char::lower_left(boxy::Weight::Normal);
        let left_branch_done_str = boxy::Char::lower_left(boxy::Weight::Normal).to_string();
        let left_node = boxy::Char::left_half(boxy::Weight::Normal);
        let down_branch = boxy::Char::down_tee(boxy::Weight::Normal);
        let down_branch_str = boxy::Char::down_tee(boxy::Weight::Normal).to_string();
        let branch_v = boxy::Char::vertical(boxy::Weight::Normal).to_string();

        if details {
            println!(
                "{}{}{}",
                title_corner,
                title_side_h.repeat(sims_mod.name.len() + 2),
                title_corner.rotate_cw(1)
            );
            println!(
                "{} {} {}",
                title_side_v,
                if let Some(result) = &verification_result {
                    if result.verification_passed() {
                        sims_mod.name.green()
                    } else {
                        sims_mod.name.red()
                    }
                    .bold()
                } else {
                    sims_mod.name.bold()
                },
                title_side_v
            );
            println!(
                "{}{}{}",
                title_branch,
                title_side_h.repeat(sims_mod.name.len() + 2),
                title_corner.rotate_cw(2)
            );
            println!(
                "{}{}{} {}",
                left_branch_more,
                left_node,
                "Version:".bold(),
                sims_mod.version
            );
            println!(
                "{}{}{} {}",
                left_branch_more,
                left_node,
                "Mod ID:".bold(),
                sims_mod.id
            );
            println!(
                "{}{}{} {}",
                left_branch_more,
                left_node,
                "Updated:".bold(),
                sims_mod.updated
            );
            println!(
                "{}{}{} {}",
                left_branch_more,
                left_node,
                "Source:".bold(),
                sims_mod.source_url
            );
            println!(
                "{}{}{} {}",
                if tags.as_ref().map(|t| !t.is_empty()).unwrap_or(false)
                    || verification_result.is_some()
                {
                    left_branch_more
                } else {
                    left_branch_done
                },
                left_node,
                "Subdirectory:".bold(),
                sims_mod.directory
            );
            if let Some(tags) = tags {
                if !tags.is_empty() {
                    println!(
                        "{}{}{}{}",
                        if verification_result.is_some() {
                            left_branch_more
                        } else {
                            left_branch_done
                        },
                        down_branch,
                        left_node,
                        "Tags:".bold()
                    );
                    let mut tags = tags.iter().peekable();
                    let mut first = true;
                    while let Some(tag) = tags.next() {
                        println!(
                            "{}{}{}{}{}",
                            if verification_result.is_some() {
                                &branch_v
                            } else {
                                " "
                            },
                            if first { &left_branch_done_str } else { " " },
                            if first {
                                if tags.peek().is_some() {
                                    &down_branch_str
                                } else {
                                    ""
                                }
                            } else {
                                if tags.peek().is_some() {
                                    &left_branch_more_str
                                } else {
                                    &left_branch_done_str
                                }
                            },
                            left_node,
                            tag
                        );
                        if first {
                            first = false;
                        }
                    }
                }
            }
            if let Some(results) = verification_result {
                let hashes = hashes.unwrap();
                let no_children = results.matching_files.is_empty()
                    && results.missing_files.is_empty()
                    && results.new_files.is_empty()
                    && results.changed_files.is_empty();
                println!(
                    "{}{}{}{} {}",
                    left_branch_done,
                    if no_children {
                        String::new()
                    } else {
                        down_branch.to_string()
                    },
                    left_node,
                    "Verification:".bold(),
                    if results.verification_passed() {
                        "PASSED".green()
                    } else {
                        "FAILED".red()
                    }
                    .bold()
                );

                if !results.matching_files.is_empty() {
                    let no_children = results.missing_files.is_empty()
                        && results.new_files.is_empty()
                        && results.changed_files.is_empty();
                    println!(
                        " {}{}{}{}:",
                        if no_children {
                            left_branch_done
                        } else {
                            left_branch_more
                        },
                        down_branch,
                        left_node,
                        "Matching".green()
                    );
                    let mut matching = results.matching_files.iter().peekable();
                    let mut first = true;
                    while let Some(matched) = matching.next() {
                        println!(
                            " {}{}{}{}{} ({})",
                            if no_children { " " } else { &branch_v },
                            if first { &left_branch_done_str } else { " " },
                            if first {
                                if matching.peek().is_some() {
                                    &down_branch_str
                                } else {
                                    ""
                                }
                            } else {
                                if matching.peek().is_some() {
                                    &left_branch_more_str
                                } else {
                                    &left_branch_done_str
                                }
                            },
                            left_node,
                            matched.display(),
                            hashes
                                .get(matched)
                                .expect_or_log("Failed to find match in hashes")
                        );
                        if first {
                            first = false;
                        }
                    }
                }

                if !results.missing_files.is_empty() {
                    let no_children =
                        results.new_files.is_empty() && results.changed_files.is_empty();
                    println!(
                        " {}{}{}{}:",
                        if no_children {
                            left_branch_done
                        } else {
                            left_branch_more
                        },
                        down_branch,
                        left_node,
                        "Missing".red()
                    );
                    let mut missing_iter = results.missing_files.iter().peekable();
                    let mut first = true;
                    while let Some(missing) = missing_iter.next() {
                        println!(
                            " {}{}{}{}{} ({})",
                            if no_children { " " } else { &branch_v },
                            if first { &left_branch_done_str } else { " " },
                            if first {
                                if missing_iter.peek().is_some() {
                                    &down_branch_str
                                } else {
                                    ""
                                }
                            } else {
                                if missing_iter.peek().is_some() {
                                    &left_branch_more_str
                                } else {
                                    &left_branch_done_str
                                }
                            },
                            left_node,
                            missing.display(),
                            hashes
                                .get(missing)
                                .expect_or_log("Failed to find match in hashes")
                        );
                        if first {
                            first = false;
                        }
                    }
                }

                if !results.new_files.is_empty() {
                    let no_children = results.changed_files.is_empty();
                    println!(
                        " {}{}{}{}:",
                        if no_children {
                            left_branch_done
                        } else {
                            left_branch_more
                        },
                        down_branch,
                        left_node,
                        "New".blue()
                    );
                    let mut new_iter = results.new_files.iter().peekable();
                    let mut first = true;
                    while let Some((new_path, hash)) = new_iter.next() {
                        println!(
                            " {}{}{}{}{} ({})",
                            if no_children { " " } else { &branch_v },
                            if first { &left_branch_done_str } else { " " },
                            if first {
                                if new_iter.peek().is_some() {
                                    &down_branch_str
                                } else {
                                    ""
                                }
                            } else {
                                if new_iter.peek().is_some() {
                                    &left_branch_more_str
                                } else {
                                    &left_branch_done_str
                                }
                            },
                            left_node,
                            new_path.display(),
                            hash
                        );
                        if first {
                            first = false;
                        }
                    }
                }

                if !results.changed_files.is_empty() {
                    println!(
                        " {}{}{}{}:",
                        left_branch_done,
                        down_branch,
                        left_node,
                        "Changed".yellow()
                    );
                    let mut changed_files = results.changed_files.iter().peekable();
                    let mut first = true;
                    while let Some((changed, new_hash)) = changed_files.next() {
                        println!(
                            "  {}{}{}{} ({} -> {})",
                            if first { &left_branch_done_str } else { " " },
                            if first {
                                if changed_files.peek().is_some() {
                                    &down_branch_str
                                } else {
                                    ""
                                }
                            } else {
                                if changed_files.peek().is_some() {
                                    &left_branch_more_str
                                } else {
                                    &left_branch_done_str
                                }
                            },
                            left_node,
                            changed.display(),
                            hashes
                                .get(changed)
                                .expect_or_log("Failed to find match in hashes"),
                            new_hash
                        );
                        if first {
                            first = false;
                        }
                    }
                }
            }
        } else {
            println!(
                "- {} ({})",
                if let Some(result) = &verification_result {
                    if result.verification_passed() {
                        sims_mod.name.green()
                    } else {
                        sims_mod.name.red()
                    }
                    .bold()
                } else {
                    sims_mod.name.bold()
                },
                sims_mod.version
            );
        }
    }

    Ok(())
}
