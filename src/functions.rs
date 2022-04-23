use clap::ArgMatches;
use regex::Regex;
use reqwest::blocking::Client;
use skim::prelude::*;
use std::error::Error;
use std::io::Cursor;
use std::path::Path;
use std::{env, process::Command};

use crate::{Defaults, Infos, Response};

fn get_fuzzy_result(search_response: String) -> Vec<Arc<dyn SkimItem>> {
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
    skim_output.selected_items
}

fn clone(owner_repo: &str, full_path: &Path, matches: &ArgMatches) -> Result<(), Box<dyn Error>> {
    let url = format!("https://github.com/{}", owner_repo);

    let git_args = match matches.values_of("git args") {
        Some(args) => args.collect::<Vec<_>>(),
        None => Vec::new(),
    };
    let command = Command::new("git")
        .arg("clone")
        .arg(url)
        .arg(full_path)
        // .arg("--")
        .args(git_args)
        .spawn();

    if let Ok(mut child) = command {
        child.wait().expect("Child process wasn't running");
    }
    Ok(())
}

fn get_api_response(client: Client, url: String) -> Response {
    let intern_response: Response =
        serde_json::from_str(&client.get(url).send().unwrap().text().unwrap()).unwrap();
    intern_response
}

pub fn clone_all(
    repos: Vec<Arc<dyn SkimItem>>,
    matches: &ArgMatches,
    defaults: Defaults,
) -> Result<(), Box<dyn Error>> {
    let current_dir = env::current_dir().unwrap();
    let mut path = matches.value_of("path").unwrap_or("").to_owned();
    if path.is_empty() {
        if let Some(defaults) = defaults.defaults {
            if let Some(default_path) = defaults.clone_path {
                path = default_path;
                if path.contains('~') {
                    panic!("Default path cannot contain a `~` ");
                }
            }
        }
    }
    if path.is_empty() {
        path = current_dir.to_str().unwrap().to_owned();
    }
    // // For each item selected clone the repo with the github cli
    for item in repos.iter() {
        let re_owner_repo = Regex::new(r"[^\s]+").unwrap();
        let re_repo = Regex::new(r"/[^\s]+").unwrap();
        let owner_repo = &re_owner_repo.captures(&item.output()).unwrap()[0].to_string();
        let repo = &re_repo.captures(owner_repo).unwrap()[0].to_string()[1..];
        if path.chars().last().unwrap() == '/' {
            path = path.chars().take(path.len() - 1).collect();
        }
        let full_path = &format!("{path}/{repo}");

        clone(owner_repo, &Path::new(full_path), matches)?
    }
    Ok(())
}

pub fn get_repos(
    matches: &ArgMatches,
    defaults: &Defaults,
    client: Client,
) -> Vec<Arc<dyn SkimItem>> {
    let limit = matches.value_of("limit").unwrap_or("30");
    let repo = matches.value_of("repository").unwrap_or("");

    if matches.is_present("owner search") && !matches.is_present("repository") {
        let search_owner = matches.value_of("owner search").unwrap();
        let users = match get_api_response(
            client.clone(),
            format!("https://api.github.com/search/users?q={search_owner}&per_page={limit}"),
        ) {
            Response::Direct(_) => panic!("Should never happen"),
            Response::Search(search_response) => match search_response.items {
                Infos::Repos(_) => panic!("Should never happen"),
                Infos::Users(users) => users,
            },
        };
        let user = get_fuzzy_result(users.to_string().to_string())[0]
            .output()
            .to_string();
        let repos = match get_api_response(
            client,
            format!("https://api.github.com/users/{user}/repos?per_page={limit}"),
        ) {
            Response::Direct(repos) => repos,
            Response::Search(_) => panic!("This should never happen"),
        };
        get_fuzzy_result(repos.to_string())
    } else if matches.is_present("owner search") && matches.is_present("repository") {
        let search_owner = matches.value_of("owner search").unwrap();
        let repo = matches.value_of("repository").unwrap();
        let users = match get_api_response(
            client.clone(),
            format!("https://api.github.com/search/users?q={search_owner}&per_page={limit}"),
        ) {
            Response::Direct(_) => panic!("Should never happen"),
            Response::Search(search_response) => match search_response.items {
                Infos::Repos(_) => panic!("Should never happen"),
                Infos::Users(users) => users,
            },
        };
        let user = get_fuzzy_result(users.to_string().to_string())[0]
            .output()
            .to_string();
        let repos = match get_api_response(
            client,
            format!("https://api.github.com/search/repositories?q={user}/{repo}&per_page={limit}"),
        ) {
            Response::Direct(_) => panic!("This should never happen"),
            Response::Search(search_response) => match search_response.items {
                Infos::Repos(repos) => repos,
                Infos::Users(_) => panic!(),
            },
        };
        get_fuzzy_result(repos.to_string())
    } else if matches.is_present("repository") && !matches.is_present("owner") {
        let repos = match get_api_response(
            client,
            format!("https://api.github.com/search/repositories?q={repo}&per_page={limit}"),
        ) {
            Response::Direct(_) => panic!("This should never happen"),
            Response::Search(search_response) => match search_response.items {
                Infos::Repos(repos) => repos,
                Infos::Users(_) => panic!("Should never happen"),
            },
        };
        get_fuzzy_result(repos.to_string())
    } else if matches.is_present("owner") && !matches.is_present("repository") {
        let owner = matches.value_of("owner").unwrap();
        let search_response = match get_api_response(
            client,
            format!("https://api.github.com/users/{owner}/repos?per_page={limit}"),
        ) {
            Response::Direct(repos) => repos,
            Response::Search(_) => panic!("Expected repo list but got a search result"),
        };
        get_fuzzy_result(search_response.to_string())
    } else if matches.is_present("owner") && matches.is_present("repository") {
        let owner = matches.value_of("owner").unwrap();
        let search_response = match get_api_response(
            client,
            format!("https://api.github.com/search/repositories?q={owner}/{repo}&per_page={limit}"),
        ) {
            Response::Direct(_) => panic!("This should never happen"),
            Response::Search(search_response) => match search_response.items {
                Infos::Repos(repos) => repos,
                Infos::Users(_) => panic!("Should never happen"),
            },
        };
        get_fuzzy_result(search_response.to_string())
    } else {
        let default_username = match &defaults.defaults {
            Some(defaults) => match &defaults.username {
                Some(username) => username,
                None => panic!("No default username provided. You must give something to search on. Check `grc --help` "),
            },
            None => panic!("No default username provided. You must give something to search on. Check `grc --help` "),
        };
        let search_response = match get_api_response(
            client,
            format!("https://api.github.com/users/{default_username}/repos?per_page={limit}"),
        ) {
            Response::Direct(repos) => repos,
            Response::Search(_) => panic!("Expected repo list but got a search result"),
        };
        get_fuzzy_result(search_response.to_string())
    }
}
