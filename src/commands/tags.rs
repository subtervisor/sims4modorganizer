use colored::Colorize;
use sea_orm::{prelude::*, Condition, IntoActiveModel};
use tracing::info;

use crate::entities::{prelude::*, *};

pub async fn tags(delete: Option<String>, tags: Option<Vec<String>>) -> crate::Result<()> {
    let db = crate::util::open_database().await?;

    if let Some(to_delete) = delete {
        info!("Deleting tag: {}", to_delete);
        if let Some(tag_model) = Tag::find()
            .filter(Condition::any().add(tag::Column::Tag.eq(&to_delete)))
            .one(&db)
            .await?
        {
            tag_model.into_active_model().delete(&db).await?;
            return Ok(());
        } else {
            eprintln!("Tag not found: {}", to_delete);
            return Err(std::io::Error::from(std::io::ErrorKind::NotFound).into());
        }
    }

    let tags_and_mods: Vec<(tag::Model, Vec<sims_mod::Model>)> = if let Some(mut tags) = tags {
        Tag::find().filter(
            tags.drain(..)
                .fold(Condition::any(), |c, t| c.add(tag::Column::Tag.eq(t))),
        )
    } else {
        Tag::find()
    }
    .find_with_related(SimsMod)
    .all(&db)
    .await?;

    for (tag, mods) in tags_and_mods.iter() {
        let title_corner = boxy::Char::upper_left(boxy::Weight::Thick);
        let title_side_h = boxy::Char::horizontal(boxy::Weight::Thick).to_string();
        let title_side_v = boxy::Char::vertical(boxy::Weight::Thick);
        let title_branch = boxy::Char::right_tee(boxy::Weight::Thick).down(boxy::Weight::Normal);
        let left_branch_more_str = boxy::Char::right_tee(boxy::Weight::Normal).to_string();
        let left_branch_done_str = boxy::Char::lower_left(boxy::Weight::Normal).to_string();
        let left_node = boxy::Char::left_half(boxy::Weight::Normal);

        let tag = tag.tag.clone();

        println!(
            "{}{}{}",
            title_corner,
            title_side_h.repeat(tag.len() + 2),
            title_corner.rotate_cw(1)
        );
        println!("{} {} {}", title_side_v, tag.bold(), title_side_v);
        println!(
            "{}{}{}",
            title_branch,
            title_side_h.repeat(tag.len() + 2),
            title_corner.rotate_cw(2)
        );

        let mut mods = mods.iter().peekable();
        while let Some(current_mod) = mods.next() {
            println!(
                "{}{}{}",
                if mods.peek().is_some() {
                    &left_branch_more_str
                } else {
                    &left_branch_done_str
                },
                left_node,
                current_mod.name
            );
        }
    }
    Ok(())
}
