use clap::{App, Arg};
use regex::Regex;
use skim::prelude::*;
use std::io::Cursor;
use std::process::Command;

#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

#[allow(non_snake_case)]
#[derive(Deserialize, Debug)]
struct GhResponse {
    // Many of these fields are here just in case I want to use them later but are currently
    // redundant
    name: String,
    nameWithOwner: String,
    updatedAt: String,
    isPrivate: bool,
    isArchived: bool,
    isFork: bool,
    isEmpty: bool,
    description: String,
}

#[derive(Deserialize, Debug)]
#[serde(transparent)]
struct GhResponses {
    responses: Vec<GhResponse>,
}
impl GhResponses {
    fn get_output(&self, filter_flags: FilterFlags) -> String {
        let mut return_string = String::new();
        for gh_response in &self.responses {
            if gh_response.isPrivate == filter_flags.only_private
                || gh_response.isPrivate != filter_flags.only_public
            {
                let test = format!(
                    "{: <30}   {} {}",
                    gh_response.nameWithOwner.clone(),
                    &gh_response.description,
                    "\n",
                );
                return_string.push_str(&test)
            }
        }
        return_string
    }
}

struct FilterFlags {
    only_private: bool,
    only_public: bool,
}
impl Default for FilterFlags {
    fn default() -> Self {
        Self {
            only_public: false,
            only_private: false,
        }
    }
}

fn main() -> Result<(), std::io::Error> {
    // Create the app with arguments
    let matches = App::new("github-repo-clone")
        .version("0.1.2")
        .author("Jared Moulton <jaredmoulton3@gmail.com>")
        .about("Scripts the usage of the github cli to make cloning slightly more convenient")
        .setting(clap::AppSettings::TrailingVarArg)
        .arg(
            Arg::new("owner")
                .short('o')
                .long("owner")
                .about("The github owner to search though")
                .required(false)
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
                .conflicts_with("private")
                .about("Show only public repositories"),
        )
        .arg(
            Arg::new("private")
                .long("private")
                .about("Show only private repositories"),
        )
        .arg(Arg::new("git args").multiple_values(true))
        .get_matches();

    // Parse the arguments
    let mut filter_flags = FilterFlags::default();
    if matches.is_present("public") {
        filter_flags.only_public = true;
    }
    if matches.is_present("private") {
        filter_flags.only_private = true;
    }

    // Fill the list args
    let arg_owner = matches.value_of("owner").unwrap_or("");
    let arg_limit = matches.value_of("limit").unwrap_or("100");
    let mut list_args: Vec<&str> = vec![arg_owner];
    list_args.push("-L");
    list_args.push(arg_limit);

    // Execute the gh cli
    let gh_output = Command::new("gh")
        .args(&["repo", "list"])
        .args(list_args)
        .args(&[
            "--json",
            "name",
            "--json",
            "nameWithOwner",
            "--json",
            "updatedAt",
            "--json",
            "isPrivate",
            "--json",
            "isArchived",
            "--json",
            "isFork",
            "--json",
            "isEmpty",
            "--json",
            "description",
        ])
        .output()?;

    let gh_responses: GhResponses =
        serde_json::from_str(std::str::from_utf8(&gh_output.stdout).unwrap()).unwrap();

    // Instantiate the fuzzy finder on the output
    let options = SkimOptionsBuilder::default()
        .height(Some("50%"))
        .multi(false)
        .color(Some("bw"))
        .build()
        .unwrap();
    let item_reader = SkimItemReader::default();
    let item = item_reader.of_bufread(Cursor::new(gh_responses.get_output(filter_flags)));
    let skim_output = Skim::run_with(&options, Some(item)).unwrap();
    if skim_output.is_abort {
        println!("No selection made");
        std::process::exit(1);
    }
    let selected_item = skim_output.selected_items;

    // For each item selected clone the repo with the github cli
    for item in selected_item.iter() {
        let re_owner_repo = Regex::new(r"[^\s]+").unwrap();
        let re_repo = Regex::new(r"/[^\s]+").unwrap();
        let owner_repo = &re_owner_repo.captures(&item.output()).unwrap()[0].to_string();
        let repo = &re_repo.captures(owner_repo).unwrap()[0].to_string()[1..];
        // Get the git args
        let args_git = match matches.values_of("git args") {
            Some(args) => args.collect::<Vec<_>>(),
            None => Vec::new(),
        };
        println!("cloning into {}", repo);
        Command::new("gh")
            .args(&["repo", "clone", owner_repo, repo])
            .arg("--")
            .args(&args_git)
            .output()?;
    }
    Ok(())
}
