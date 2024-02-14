# sims4modorganizer

A simple mod manager for Sims 4 mods, written in Rust. Tested on macOS.

## Description

This is designed more around managing mods from web sources without an elegant update API, so it makes no effort to have auto-update or any such functionality. Instead, it is intended to provide a database of metadata for mods to ease tracking and updating them manually. The source URL is stored, along with the latest metadata. Package and script files are hashed to detect changes, and when changed, they can be updated (along with their metadata).

## Getting Started

### Installing

```bash
git clone https://github.com/subtervisor/sims4modorganizer
cd sims4modorganizer
cargo install --path .
```

### Usage

To get started, you want to initialize the database with `sims4modorganizer initialize`. This will create an empty database for use. You can also use the `-f`/`--force` paramater to delete an existing database if it's corrupted or you want to start fresh.

The intended flow is one where you add mods to the mod folder with each in its own dedicated subdirectory. Packages and script files should all be one level deep in the mod folder. Each mod folder is considered a separate mod for tracking purposes. The `mod_data` folder is ignored, as this is shared between mods.

The `scan` subcommand handles mod verification and updates. By default, it will scan the mod directory and compare to the database state, showing new, deleted, and existing mods. With the `--verify`/`-v` flag, it will additionally check file hashes, scanning for missing, new, or updated files. The `--fix`/`-f` flag will enable interactive update of the mod database. It will ask the user if they want to add newly-found mods, querying them for the metadata, or delete missing ones. If verification is also enabled, it will ask for updated metadata for mods with changed hash data and update them. The `--sync-hashes`/`-s` option, which is mutually exclusive with the `--fix`/`-f` flag, will non-interactively scan hash data and update existing hashes.

The `list` subcommand shows mods, optionally filtering them by tag with the `--tags`/`-t` option, which accepts a comma-separated list of tags. The `--verify`/`-v` flag enables scanning file hashes and showing verification status. The `--details`/`-d` flag enables showing more than the mod name/version/validation status, printing all data including mod database ID, source URL, update timestamp, tags, and file verification details.

The `tags` subcommand shows existing tags and offers the ability to delete them. Without arguments, it lists all tags and their associated mods. There are two mutually-exclusive options: `--delete`/`-d` deletes the specified tag (and doesn't show anything otherwise), and `--tags`/`-t` only shows the specified tags in the list displayed.

The `edit` subcommand allows you to edit mods. There are two ways to edit the menu. First, the menu-driven editor via `--interactive`/`-i`, which launches a menu-driven editor to update mod metadata. You can find mods via the full list or filter by tag, and the editor can be used to edit multiple mods in a single invocation via the menus. Alternatively, you can use the `--mod-id`/`-m` option to specify a mod ID and one or more of the `--name`/`-n`, `--source-url`/`-s`, `--tags`/`-t`, or `--mod-version`/`-v` options to assign mod metadata non-interactively. The mod ID is shown both via the `list` command and the interactive menu view.