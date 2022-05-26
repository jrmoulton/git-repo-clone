use crate::get_fuzzy_result;
use anyhow::{anyhow, Result};
use clap::ArgMatches;
use reqwest::blocking::Client;
use serde_derive::{Deserialize, Serialize};

pub trait ExternTypeToString {
    fn to_string(&self) -> String;
}
impl ExternTypeToString for Vec<RepoInfo> {
    fn to_string(&self) -> String {
        let mut return_string = String::new();
        self.iter().for_each(|repo| {
            return_string.push_str(&format!(
                "{: <30}   {}\n",
                &repo.full_name,
                &repo.description.as_ref().unwrap_or(&"".to_string())
            ))
        });
        return_string
    }
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum Response {
    Direct(Vec<RepoInfo>),
    Search(SearchResponse),
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
    Repos(Vec<RepoInfo>),
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct RepoInfo {
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

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct DefaultConfig {
    pub clone_path: Option<String>,
    pub username: Option<String>,
}

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

pub fn get_repo_name(matches: &ArgMatches, defaults: &DefaultConfig) -> Result<Vec<RepoInfo>> {
    let limit = matches.value_of("limit").unwrap_or("100");
    let repo = matches.value_of("repository").unwrap_or("");
    let client = reqwest::blocking::Client::builder()
        .user_agent(APP_USER_AGENT)
        .build()?;

    if matches.is_present("owner search") && !matches.is_present("repository") {
        let search_owner = matches.value_of("owner search").unwrap();
        let users = match get_api_response(
            client.clone(),
            format!("https://api.github.com/search/users?q={search_owner}&per_page={limit}"),
        ) {
            Response::Direct(_) => return Err(anyhow!("Should never happen")),
            Response::Search(search_response) => match search_response.items {
                Infos::Repos(_) => return Err(anyhow!("Should never happen")),
                Infos::Users(users) => users,
            },
        };
        let user = get_fuzzy_result(users.to_string())?;
        match get_api_response(
            client,
            format!("https://api.github.com/users/{user}/repos?per_page={limit}"),
        ) {
            Response::Direct(repos) => Ok(repos),
            Response::Search(_) => Err(anyhow!("This should never happen")),
        }
    } else if matches.is_present("owner search") && matches.is_present("repository") {
        let search_owner = matches.value_of("owner search").unwrap();
        let repo = matches.value_of("repository").unwrap();
        let users = match get_api_response(
            client.clone(),
            format!("https://api.github.com/search/users?q={search_owner}&per_page={limit}"),
        ) {
            Response::Direct(_) => return Err(anyhow!("Should never happen")),
            Response::Search(search_response) => match search_response.items {
                Infos::Repos(_) => return Err(anyhow!("Should never happen")),
                Infos::Users(users) => users,
            },
        };
        let user = get_fuzzy_result(users.to_string())?;
        match get_api_response(
            client,
            format!("https://api.github.com/search/repositories?q={user}/{repo}&per_page={limit}"),
        ) {
            Response::Direct(_) => return Err(anyhow!("This should never happen")),
            Response::Search(search_response) => match search_response.items {
                Infos::Repos(repos) => Ok(repos),
                Infos::Users(_) => Err(anyhow!("")),
            },
        }
    } else if matches.is_present("repository") && !matches.is_present("owner") {
        match get_api_response(
            client,
            format!("https://api.github.com/search/repositories?q={repo}&per_page={limit}"),
        ) {
            Response::Direct(_) => return Err(anyhow!("This should never happen")),
            Response::Search(search_response) => match search_response.items {
                Infos::Repos(repos) => Ok(repos),
                Infos::Users(_) => Err(anyhow!("Should never happen")),
            },
        }
    } else if matches.is_present("owner") && !matches.is_present("repository") {
        let owner = matches.value_of("owner").unwrap();
        match get_api_response(
            client,
            format!("https://api.github.com/users/{owner}/repos?per_page={limit}"),
        ) {
            Response::Direct(repos) => Ok(repos),
            Response::Search(_) => Err(anyhow!("Expected repo list but got a search result")),
        }
    } else if matches.is_present("owner") && matches.is_present("repository") {
        let owner = matches.value_of("owner").unwrap();
        match get_api_response(
            client,
            format!("https://api.github.com/search/repositories?q={owner}/{repo}&per_page={limit}"),
        ) {
            Response::Direct(_) => Err(anyhow!("This should never happen")),
            Response::Search(search_response) => match search_response.items {
                Infos::Repos(repos) => Ok(repos),
                Infos::Users(_) => Err(anyhow!("Should never happen")),
            },
        }
    } else {
        let default_username = match &defaults.username {
                Some(username) => username,
                None => return Err(anyhow!("No default username provided. You must give a search parameter or configure the defaults in the config file. Check `grc --help` ")),
        };
        match get_api_response(
            client,
            format!("https://api.github.com/users/{default_username}/repos?per_page={limit}"),
        ) {
            Response::Direct(repos) => Ok(repos),
            Response::Search(_) => Err(anyhow!("Expected repo list but got a search result")),
        }
    }
}

fn get_api_response(client: Client, url: String) -> Response {
    serde_json::from_str(
            &client
                .get(url)
                .send()
                .expect("Unable to send request")
                .text()
                .expect("Unable to decode the response"),
        )
        .expect("The response was not in the correct form. This should only happen if the github rest api changes")
}
