use clap::{App, Arg};
use regex::Regex;
use skim::prelude::*;
use std::io::Cursor;
use std::process::Command;
use git2::{Cred, RemoteCallbacks};
use std::env;
use std::path::Path;

#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the app with arguments
    let matches = App::new("github-repo-clone")
        .version("0.1.2")
        .author("Jared Moulton <jaredmoulton3@gmail.com>")
        .about("Scripts the usage of the github cli to make cloning slightly more convenient")
        .setting(clap::AppSettings::TrailingVarArg)
        .arg(
            Arg::new("owner")
                .about("The github owner to search though")
                .required(false)
                .takes_value(true)
                .index(1),
        )
        .arg(
            Arg::new("path")
                .short('p')
                .long("path")
                .about("The full path to clone into")
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
        .arg(
            Arg::new("bare")
            .long("bare")
            .about("Whether to clone the repository as a bare repo"),
        )
        .get_matches();

    // Parse the filter flags
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

        // Parse the path flag
        //
        // This is a closure to check if the directory already exists
        // I'm using this because I can't figure out how to use if statements inside of the match
        // arm
        let check_dir = |path: &str| {
            if std::path::Path::new(&path).is_dir() {
                format!("{}/{}", path.trim_end_matches('/'), repo)
            } else {
                path.to_owned()
            }
        };
        let path = match matches.value_of("path") {
            Some(path) => check_dir(path),
            None => repo.clone().to_string(),
        };
        if !path.is_empty() {
            println!("Cloning {} into {}", repo, path);
        } else {
            println!("Cloning into {}", repo);
        }

        // Prepare callbacks.
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_url, username_from_url, _allowed_types| {
            dbg!(username_from_url);
            Cred::ssh_key(
                username_from_url.unwrap(),
                None,
                std::path::Path::new(&format!("{}/.ssh/id_rsa", env::var("HOME").unwrap())),
                None,
                )
        });
        // Prepare fetch options.
        let mut fo = git2::FetchOptions::new();
        fo.remote_callbacks(callbacks);

        let url = format!("https://github.com/{}",owner_repo);
        git2::build::RepoBuilder::new()
        .fetch_options(fo)
        .bare(matches.is_present("bare"))
        .clone(&url, Path::new(&path))?;
    }
    Ok(())
}

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
