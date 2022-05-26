mod cli;
mod response;

use crate::{
    cli::{create_app, handle_sub_commands, OptionGiven},
    response::{DefaultConfig, ExternTypeToString},
};
use anyhow::{anyhow, Context, Result};
use clap::ArgMatches;
use regex::Regex;
use skim::prelude::*;
use std::{env, io::Cursor, path::PathBuf, process};

fn main() -> Result<()> {
    let matches = create_app();

    match handle_sub_commands(&matches)? {
        OptionGiven::Yes => return Ok(()),
        OptionGiven::No => {} //continue
    }

    let defaults = confy::load::<DefaultConfig>("grc")?;

    let repos = response::get_repo_name(&matches, &defaults)?;

    let repo = get_fuzzy_result(repos.to_string())?;

    clone(repo, &matches, defaults)?;

    Ok(())
}

fn get_fuzzy_result(search_response: String) -> Result<String> {
    let options = SkimOptionsBuilder::default()
        .height(Some("50%"))
        .multi(false)
        .color(Some("bw"))
        .build()
        .unwrap();
    let item_reader = SkimItemReader::default();
    let item = item_reader.of_bufread(Cursor::new(search_response));
    let skim_output = Skim::run_with(&options, Some(item)).unwrap();
    if skim_output.is_abort {
        return Err(anyhow!("No selection made"));
    }
    Ok(skim_output.selected_items[0].output().to_string())
}

fn clone(repo_name: String, matches: &ArgMatches, defaults: DefaultConfig) -> Result<()> {
    let mut clone_path = match matches.value_of("path") {
        Some(path) => path.to_owned(),
        None => match defaults.clone_path {
            Some(default_path) => default_path,
            None => env::current_dir()?
                .to_str()
                .context("Not a valid UTF8 path")?
                .to_owned(),
        },
    };

    // For each item selected clone the repo with the github cli
    let regex_owner_repo = Regex::new(r"[^\s]+")?;
    let regex_repo = Regex::new(r"/[^\s]+")?;

    let owner_repo = &regex_owner_repo.captures(&repo_name.output()).unwrap()[0].to_string();
    let repo = match matches.value_of("new") {
        Some(name) => name.to_owned(),
        None => regex_repo.captures(owner_repo).unwrap()[0].to_string()[1..].to_owned(),
    };

    if clone_path.ends_with('/') {
        clone_path.pop();
    }
    let full_path = PathBuf::from(format!("{clone_path}/{repo}"));

    let owner_repo: &str = owner_repo;
    let url = format!("https://github.com/{}", owner_repo);

    let git_args = match matches.values_of("git args") {
        Some(args) => args.collect::<Vec<_>>(),
        None => Vec::new(),
    };
    process::Command::new("git")
        .arg("clone")
        .arg(url)
        .arg(full_path)
        .args(git_args)
        .output()?;

    Ok(())
}
