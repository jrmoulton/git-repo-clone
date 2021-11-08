use clap::{App, Arg};
use regex::Regex;
use skim::prelude::*;
use std::io::Cursor;
use std::io::Result;
use std::process::Command;

#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

#[derive(Serialize, Deserialize, Debug)]
struct GhResponse {
    name: String,
    name_with_owner: String,
    updated_at: String,
    is_private: bool,
    is_archived: bool,
    is_fork: bool,
    is_empty: bool,
}

fn main() -> Result<()> {
    let matches = App::new("github-repo-clone")
        .version("0.1.1")
        .author("Jared Moulton <jaredmoulton3@gmail.com>")
        .about("Scripts the usage of the github cli to make cloning slightly more convenient")
        .setting(clap::AppSettings::TrailingVarArg)
        .setting(clap::AppSettings::AllowLeadingHyphen)
        .arg(
            Arg::new("account")
                .short('a')
                .long("account")
                .about("The github account to search though")
                .takes_value(true),
        )
        .arg(
            Arg::new("limit")
                .short('l')
                .long("limit")
                .about("The number of repositories to list default=100")
                .takes_value(true),
        )
        .arg(
            Arg::new("public")
                .long("public")
                .about("Show only public repositories"),
        )
        .arg(
            Arg::new("private")
                .long("private")
                .about("Show only private repositories"),
        )
        .arg(Arg::new("git args").multiple_values(true))
        .get_matches();

    let args_git = match matches.values_of("git args") {
        Some(args) => args.collect::<Vec<_>>(),
        None => Vec::new(),
    };
    let arg_account = matches.value_of("account").unwrap_or("");
    let arg_public = if matches.is_present("public") {
        "--public".to_string()
    } else {
        "".to_string()
    };
    let arg_private = if matches.is_present("private") {
        "--private".to_string()
    } else {
        "".to_string()
    };
    let arg_limit = matches.value_of("limit").unwrap_or("100");
    let arg_limit = vec!["-L", arg_limit];

    let re_account_repo = Regex::new(r"[^\s]+").unwrap();
    let re_repo = Regex::new(r"/[^\s]+").unwrap();

    let options = SkimOptionsBuilder::default()
        .height(Some("50%"))
        .multi(false)
        .color(Some("bw"))
        .build()
        .unwrap();
    let item_reader = SkimItemReader::default();

    // Commands
    let temp_args = vec![arg_account, &arg_public, &arg_private];
    let mut list_args: Vec<&str> = Vec::new();
    for arg in &temp_args {
        if !arg.is_empty() {
            list_args.push(arg);
        }
    }
    let gh_output = Command::new("gh")
        .args(&["repo", "list"])
        .args(list_args)
        .args(arg_limit)
        .output()
        .expect("Couldn't execute gh binary with args");

    let item = item_reader.of_bufread(Cursor::new(gh_output.stdout));
    let skim_output = Skim::run_with(&options, Some(item)).unwrap();
    if skim_output.is_abort {
        println!("No selection made");
        std::process::exit(1);
    }
    let selected_item = skim_output.selected_items;

    for item in selected_item.iter() {
        let account_repo = &re_account_repo.captures(&item.output()).unwrap()[0].to_string();
        let repo = &re_repo.captures(account_repo).unwrap()[0].to_string()[1..];
        if repo.is_empty() {}
        println!("cloning into {}", repo);
        Command::new("gh")
            .args(&["repo", "clone", account_repo, repo])
            .arg("--")
            .args(&args_git)
            .output()
            .expect("Couldn't execute gh binary with args");
    }

    Ok(())
}
