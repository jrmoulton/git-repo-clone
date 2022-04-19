use clap::{Arg, Command};
use git2::{Cred, RemoteCallbacks};
use regex::Regex;
use skim::prelude::*;
use std::env;
use std::io::Cursor;
use std::path::Path;

#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct GhSearchResponse {
    // Many of these fields are here just in case I want to use them later but are currently
    // redundant
    total_count: i32,
    incomplete_results: bool,
    items: RepoInfos,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct RepoInfo {
    id: i32,
    node_id: String,
    name: String,
    full_name: String,
    private: bool,
    owner: serde_json::Value,
    html_url: String,
    description: Option<String>,
    fork: bool,
    git_url: String,
    ssh_url: String,
    default_branch: String,
}

#[derive(Deserialize, Debug)]
#[serde(transparent)]
struct RepoInfos {
    repos: Vec<RepoInfo>,
}
impl ToString for RepoInfos {
    fn to_string(&self) -> String {
        let mut return_string = String::new();
        for repo in &self.repos {
            return_string.push_str(&format!(
                "{: <30}   {}\n",
                &repo.full_name,
                &repo.description.as_ref().unwrap_or(&"".to_string())
            ));
        }
        return_string
    }
}

fn get_response(client: reqwest::blocking::Client, url: String) -> String {
    client.get(url).send().unwrap().text().unwrap()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the app with arguments
    let matches = Command::new("git-repo-clone")
        .version("0.1.2")
        .author("Jared Moulton <jaredmoulton3@gmail.com>")
        .about("Mixes cloning git repositories with fuzzy finding to make cloning slightly more convenient")
        .trailing_var_arg(false)
        .arg(
            Arg::new("repository")
                .help("The repository name to search for")
                .required(false)
                .takes_value(true),
        )
        .arg(
            Arg::new("owner")
                .short('o')
                .long("owner")
                .help("The owner account to search through")
                .required(false)
                .takes_value(true),
        )
        .arg(
            Arg::new("path")
                .short('p')
                .long("path")
                .help("The full path to clone into")
                .required(false)
                .takes_value(true),
        )
        .arg(
            Arg::new("limit")
                .short('l')
                .long("limit")
                .help("The number of repositories to querry and list default=50")
                .takes_value(true),
        )
        .arg(
            Arg::new("public")
                .long("public")
                .conflicts_with("private")
                .help("Show only public repositories"),
        )
        .arg(
            Arg::new("private")
                .long("private")
                .help("Show only private repositories"),
        )
        .arg(
            Arg::new("bare")
                .long("bare")
                .help("Whether to clone the repository as a bare repo"),
        )
        .arg(Arg::new("host")
            .short('h')
            .long("host")
            .help("Define which host provider to use. [Github, Gitlab] or full url"))
        .get_matches();

    let client = reqwest::blocking::Client::builder()
        .user_agent(APP_USER_AGENT)
        .build()?;

    let limit = matches.value_of("limit").unwrap_or("50");
    let repo = matches.value_of("repository").unwrap_or("");
    let search_response: RepoInfos = if matches.is_present("repository")
        && !matches.is_present("owner")
    {
        let intern_response: GhSearchResponse = serde_json::from_str(&get_response(
            client,
            format!("https://api.github.com/search/repositories?q={repo}&per_page={limit}"),
        ))
        .unwrap();
        intern_response.items
    } else if matches.is_present("owner") && !matches.is_present("repository") {
        let owner = matches.value_of("owner").unwrap();
        serde_json::from_str(&get_response(
            client,
            format!("https://api.github.com/users/{owner}/repos?per_page={limit}"),
        ))
        .unwrap()
    } else if matches.is_present("owner") && matches.is_present("repository") {
        let owner = matches.value_of("owner").unwrap();
        let internal_response: GhSearchResponse = serde_json::from_str(&get_response(
            client,
            format!("https://api.github.com/search/repositories?q={owner}/{repo}&per_page={limit}"),
        ))
        .unwrap();
        internal_response.items
    } else {
        serde_json::from_str(&get_response(
            client,
            format!("https://api.github.com/users/jrmoulton/repos?per_page={limit}"),
        ))
        .unwrap()
    };

    // Instantiate the fuzzy finder on the output
    let options = SkimOptionsBuilder::default()
        .height(Some("50%"))
        .multi(false)
        .color(Some("bw"))
        .build()
        .unwrap();
    let item_reader = SkimItemReader::default();
    let item = item_reader.of_bufread(Cursor::new(search_response.to_string()));
    let skim_output = Skim::run_with(&options, Some(item)).unwrap();
    if skim_output.is_abort {
        println!("No selection made");
        std::process::exit(1);
    }
    let selected_item = skim_output.selected_items;

    // // For each item selected clone the repo with the github cli
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

        let url = format!("https://github.com/{}", owner_repo);
        git2::build::RepoBuilder::new()
            .fetch_options(fo)
            .bare(matches.is_present("bare"))
            .clone(&url, Path::new(&path))?;
    }
    Ok(())
}
