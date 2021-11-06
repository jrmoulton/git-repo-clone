use clap::{App, Arg};
use regex::Regex;
use skim::prelude::*;
use std::io::Cursor;
use std::io::Result;
use std::process::{Command, Stdio};

fn main() -> Result<()> {
    let matches = App::new("grc-2")
        .version("0.1.0")
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
            Arg::new("repo")
                .short('r')
                .long("repo")
                .about("The repo that you are looking for <not working>")
                .takes_value(true),
        )
        .arg(Arg::new("git").multiple_values(true))
        .get_matches();

    let git_args = match matches.values_of("git") {
        Some(args) => args.collect::<Vec<_>>(),
        None => Vec::new(), //collect::<Vec<_>>();
    };

    let arg_account = matches.value_of("account").unwrap_or("");
    println!("{}", arg_account);
    let arg_repo = matches.value_of("repo").unwrap_or("");
    let mut command = "".to_string();

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
    let gh_output = Command::new("gh")
        .args(&["repo", "list", "-l", "100"])
        .arg(arg_account)
        .output()
        .expect("Couldn't execute gh binary with args");

    let item = item_reader.of_bufread(Cursor::new(gh_output.stdout));
    let selected_item = Skim::run_with(&options, Some(item)).unwrap().selected_items;

    for item in selected_item.iter() {
        let account_repo = &re_account_repo.captures(&item.output()).unwrap()[0].to_string();
        let repo = &re_repo.captures(account_repo).unwrap()[0].to_string()[1..];
        if repo.is_empty() {
            println!("No selection made");
            std::process::exit(0);
        }
        println!("cloning into {}", repo);
        Command::new("gh")
            .args(&["repo", "clone", account_repo, repo])
            .arg("--")
            .args(&git_args)
            .output()
            .expect("Couldn't execute gh binary with args");
    }

    Ok(())
}
