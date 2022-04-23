mod functions;

use clap::{Arg, Command};

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

// TOML STUFF
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Defaults {
    defaults: Option<DefaultConfig>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
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
        .long_about("Mixes cloning git repositories with fuzzy finding to make cloning slightly more convenient. \n\nA user configuration file can be provided as ~/.config/grc/grc.toml with a default username and clone path")
        .trailing_var_arg(false)
        .arg(
            Arg::new("repository")
                .help("The repository name to search for (not required)")
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
            .help("Search for an owner if the exact name isn't known and get their repos")
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
                .help("The full path to the parent folder to clone into")
                .required(false)
                .takes_value(true),
        )
        .arg(
            Arg::new("limit")
                .short('l')
                .long("limit")
                .help("The number of repositories to query and list: default=30")
                .takes_value(true),
        )
        // .arg(
        //     Arg::new("public")
        //         .long("public")
        //         .conflicts_with("private")
        //         .help("Show only public repositories"),
        // )
        // .arg(
        //     Arg::new("private")
        //         .long("private")
        //         .help("Show only private repositories"),
        // )
        // .arg(Arg::new("host")
        //     .short('h')
        //     .long("host")
        //     .help("Define which host provider to use. [Github, Gitlab] or full url"))
        .arg(Arg::new("git args")
            .multiple_values(true)
        .help("All additional git args")
            .long_help("All additional git args. After all other options pass `--` and then the git args. Eg `grc rust -- --bare")
        )
            
        .get_matches();

    let client = reqwest::blocking::Client::builder()
        .user_agent(APP_USER_AGENT)
        .build()?;

    let defaults: Defaults = toml::from_str(
        &std::fs::read_to_string(format!("/Users/jaredmoulton/.config/grc/grc.toml")).unwrap(),
    )
    .unwrap();
    let repos = functions::get_repos(&matches, &defaults, client);
    functions::clone_all(repos, &matches, defaults)
}
