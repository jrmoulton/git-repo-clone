mod functions;

use clap::{Arg, Command};

#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum Response {
    Direct(RepoInfos),
    Search(SearchResponse),
}

// I need to be able to call to_string on this and there is no way to implement that directly on a
// Vec<RepoInfo>
#[derive(Deserialize, Debug)]
#[serde(transparent)]
pub struct RepoInfos {
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
pub struct SearchResponse {
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
            Arg::new("owner search")
            .help("Search for an owner and get their repos")
                .long("ownersearch")
                .short('O')
                .conflicts_with("owner")
                .takes_value(true)
                .required(false)
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
                .help("The number of repositories to querry and list default=30")
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

    let repo = matches.value_of("repository").unwrap_or("");
    let path = match matches.value_of("path") {
        Some(path) => check_dir(path, repo),
        None => repo.clone().to_string(),
    };

    let repos = functions::get_repos(&matches, client, repo);
    functions::clone_all(repos, path, matches.is_present("bare"))
}

// A function to check if the directory already exists I'm using this because I can't figure out
// how to use if statements inside of the match arm
fn check_dir(path: &str, repo: &str) -> String {
    if std::path::Path::new(&path).is_dir() {
        format!("{}/{}", path.trim_end_matches('/'), repo)
    } else {
        path.to_owned()
    }
}
