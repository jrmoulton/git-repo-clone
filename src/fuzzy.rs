use clap::ArgMatches;
use git2::{Cred, RemoteCallbacks};
use regex::Regex;
use reqwest::blocking::Client;
use skim::prelude::*;
use std::env;
use std::error::Error;
use std::io::Cursor;
use std::path::Path;

use crate::{Infos, Response};

pub fn get_fuzzy_result(search_response: String) -> Vec<Arc<dyn SkimItem>> {
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

pub fn clone(owner_repo: &str, path: String, bare: bool) -> Result<(), Box<dyn Error>> {
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
        .bare(bare)
        .clone(&url, Path::new(&path))?;
    Ok(())
}

pub fn get_api_response(client: Client, url: String) -> Response {
    let intern_response: Response =
        serde_json::from_str(&client.get(url).send().unwrap().text().unwrap()).unwrap();
    intern_response
    // let results = match intern_response {
    //     Response::Direct(repos) => repos,
    //     Response::Search(search_response) => search_response.items,
    // };

    // if results.repos.is_empty() {
    //     println!("No search results found");
    //     std::process::exit(1);
    // } else {
    //     results
    // }
}

pub fn clone_all(
    repos: Vec<Arc<dyn SkimItem>>,
    path: String,
    bare: bool,
) -> Result<(), Box<dyn Error>> {
    // // For each item selected clone the repo with the github cli
    for item in repos.iter() {
        let re_owner_repo = Regex::new(r"[^\s]+").unwrap();
        let re_repo = Regex::new(r"/[^\s]+").unwrap();
        let owner_repo = &re_owner_repo.captures(&item.output()).unwrap()[0].to_string();
        let repo = &re_repo.captures(owner_repo).unwrap()[0].to_string()[1..];

        if !path.is_empty() {
            println!("Cloning {} into {}", repo, path);
        } else {
            println!("Cloning into {}", repo);
        }

        clone(owner_repo, path.clone(), bare)?
    }
    Ok(())
}

pub fn get_repos(matches: &ArgMatches, client: Client, repo: &str) -> Vec<Arc<dyn SkimItem>> {
    let limit = matches.value_of("limit").unwrap_or("30");

    if matches.is_present("owner search") {
        let search_owner = matches.value_of("owner search").unwrap();
        let users = match get_api_response(
            client.clone(),
            format!("https://api.github.com/search/users?q={search_owner}&per_page={limit}"),
        ) {
            Response::Direct(_) => panic!(),
            Response::Search(search_response) => match search_response.items {
                Infos::Repos(_) => panic!(),
                Infos::Users(users) => users,
            },
        };
        let user = get_fuzzy_result(users.to_string().to_string())[0]
            .output()
            .to_string();
        let repos = match get_api_response(
            client,
            format!("https://api.github.com/search/repositories?q={user}&per_page={limit}"),
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
                Infos::Users(_) => panic!(),
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
                Infos::Users(_) => panic!(),
            },
        };
        get_fuzzy_result(search_response.to_string())
    } else {
        let search_response = match get_api_response(
            client,
            format!("https://api.github.com/users/jrmoulton/repos?per_page={limit}"),
        ) {
            Response::Direct(repos) => repos,
            Response::Search(_) => panic!("Expected repo list but got a search result"),
        };
        get_fuzzy_result(search_response.to_string())
    }
}
