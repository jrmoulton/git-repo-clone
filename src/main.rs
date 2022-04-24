use clap::ArgMatches;
use clap::{Arg, Command};
use regex::Regex;
use reqwest::blocking::Client;
use skim::prelude::*;
use std::error::Error;
use std::io::Cursor;
use std::path::Path;
use std::{env, process};

#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum Response {
    Direct(RepoInfos),
    Search(SearchResponse),
}

// I need to be able to call to_string on this and there is no way to implement that directly on a
// Vec<RepoInfo>
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

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct SearchResponse {
    total_count: i32,
    incomplete_results: bool,
    items: Infos,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum Infos {
    Users(UserInfos),
    Repos(RepoInfos),
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct RepoInfo {
    name: String,
    full_name: String,
    description: Option<String>,
    private: bool,
    fork: bool,
    html_url: String,
    git_url: String,
    ssh_url: String,
    default_branch: String,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
#[serde(transparent)]
struct UserInfos {
    users: Vec<UserInfo>,
}
impl ToString for UserInfos {
    fn to_string(&self) -> String {
        let mut return_string = String::new();
        for user in &self.users {
            return_string.push_str(&format!("{}\n", &user.login));
        }
        return_string
    }
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct UserInfo {
    login: String,
    id: i32,
    html_url: String,
}

#[allow(dead_code)]
#[derive(Debug, Default, Deserialize, Serialize)]
struct DefaultConfig {
    clone_path: Option<String>,
    username: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the app with arguments
    let matches = Command::new("git-repo-clone")
        .version("0.2.0")
        .author("Jared Moulton <jaredmoulton3@gmail.com>")
        .about("Mixes cloning git repositories with fuzzy finding to make cloning slightly more convenient")
        .trailing_var_arg(false)
        .disable_help_subcommand(true)
        .arg(
            Arg::new("repository")
                .allow_hyphen_values(false)
                .help("The repository name to search for")
                .takes_value(true),
        )
        .arg(
            Arg::new("owner")
                .short('o')
                .long("owner")
                .help("The owner account to search through")
                .takes_value(true),
        )
        .arg(
            Arg::new("owner search")
            .help("Search for an owner if the exact name isn't known and get their repos")
                .long("ownersearch")
                .short('O')
                .conflicts_with("owner")
                .takes_value(true)
        )
        .arg(
            Arg::new("path")
                .short('p')
                .long("path")
                .help("The full path to the parent folder to clone into")
                .takes_value(true),
        )
        .arg(
            Arg::new("limit")
                .short('l')
                .long("limit")
                .help("The number of repositories to query and list: default=30")
                .takes_value(true),
        )
            .arg(
            Arg::new("new name")
                .short('n')
                .long("new")
                .help("A custom name for renaming the repository")
                .takes_value(true)
        )
        .arg(Arg::new("git args")
            .multiple_values(true)
            .allow_hyphen_values(true)
            .takes_value(true)
            .long("git")
            .short('g')
            .help("All additional git args. After all other options pass `-g` and then the git args. \
                Eg `grc rust -g --bare` ")
        )
        .subcommand(Command::new("default-config")
            .args_conflicts_with_subcommands(true)
            .about("Configure your defaults")
            .arg(Arg::new("username")
                .takes_value(true)
                .required(true)
                .short('u')
                .long("username")
                .help("The default username to search for when no other search parameters are given")
            )
            .arg(Arg::new("clone path")
                .takes_value(true)
                .required(true)
                .short('p')
                .long("path")
                .help("The default path to clone repositories into when none is specified. \
                        If this is blank and none is specified it will clone into the current folder")
            )
        )
        .get_matches();

    match matches.subcommand() {
        Some(("default-config", sub_m)) => {
            let username = match sub_m.value_of("username") {
                Some(name) => Some(name.to_string()),
                None => None,
            };
            let clone_path = match sub_m.value_of("clone path") {
                Some(path) => Some(path.to_string()),
                None => None,
            };
            let config = DefaultConfig {
                username,
                clone_path,
            };
            confy::store("grc", config)?;
            println!("Configuration has been stored");
            std::process::exit(0);
        }
        _ => {}
    }

    let client = reqwest::blocking::Client::builder()
        .user_agent(APP_USER_AGENT)
        .build()?;

    let defaults: DefaultConfig = confy::load("grc").unwrap();
    let repos = get_repos(&matches, &defaults, client);
    clone_all(repos, &matches, defaults)
}

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
    let command = process::Command::new("git")
        .arg("clone")
        .arg(url)
        .arg(full_path)
        .args(git_args)
        .spawn();

    if let Ok(mut child) = command {
        child.wait().expect("Child process wasn't running");
    }
    Ok(())
}

fn get_api_response(client: Client, url: String) -> Response {
    Response::from(
        serde_json::from_str(
            &client
                .get(url)
                .send()
                .expect("Unable to send request")
                .text()
                .expect("Unable to decode the response"),
        )
        .expect("The response was not in the correct form. This should only happen if the github rest api changes"),
    )
}

fn clone_all(
    repos: Vec<Arc<dyn SkimItem>>,
    matches: &ArgMatches,
    defaults: DefaultConfig,
) -> Result<(), Box<dyn Error>> {
    let current_dir = env::current_dir().unwrap();
    let mut path = matches.value_of("path").unwrap_or("").to_owned();
    if path.is_empty() {
        if let Some(default_path) = defaults.clone_path {
            path = default_path;
            if path.contains('~') {
                panic!("Default path cannot contain a `~` ");
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
        let repo = match matches.value_of("new") {
            Some(name) => name.to_owned(),
            None => re_repo.captures(owner_repo).unwrap()[0].to_string()[1..].to_owned(),
        };

        if path.chars().last().unwrap() == '/' {
            path = path.chars().take(path.len() - 1).collect();
        }
        let full_path = &format!("{path}/{repo}");

        clone(owner_repo, &Path::new(full_path), matches)?
    }
    Ok(())
}

fn get_repos(
    matches: &ArgMatches,
    defaults: &DefaultConfig,
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
        let default_username = match &defaults.username {
                Some(username) => username,
                None => panic!("No default username provided. You must give a search parameter or configure the defaults in the config file. Check `grc --help` "),
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
