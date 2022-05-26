use crate::response::DefaultConfig;
use anyhow::Result;
use clap::{Arg, ArgMatches, Command};

pub enum OptionGiven {
    Yes,
    No,
}

pub fn create_app() -> ArgMatches {
    Command::new("git-repo-clone")
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
                .help("The number of repositories to query and list: default=100")
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
            .arg_required_else_help(true)
            .args_conflicts_with_subcommands(true)
            .about("Configure your defaults")
            .arg(Arg::new("username")
                .takes_value(true)
                .short('u')
                .long("username")
                .help("The default username to search for when no other search parameters are given")
            )
            .arg(Arg::new("clone path")
                .takes_value(true)
                .short('p')
                .long("path")
                .help("The default path to clone repositories into when none is specified. \
                        If this is blank and none is specified it will clone into the current folder")
            )
        )
        .get_matches()
}

pub fn handle_sub_commands(matches: &ArgMatches) -> Result<OptionGiven> {
    match matches.subcommand() {
        Some(("default-config", sub_m)) => {
            let defaults: DefaultConfig = confy::load("grc")?;
            let username = match sub_m.value_of("username") {
                Some(name) => Some(name.to_string()),
                None => defaults.username,
            };
            let clone_path = match sub_m.value_of("clone path") {
                Some(path) => Some(path.to_string()),
                None => defaults.clone_path,
            };
            let config = DefaultConfig {
                username,
                clone_path,
            };
            confy::store("grc", config)?;
            println!("Configuration has been stored");
            Ok(OptionGiven::Yes)
        }
        _ => Ok(OptionGiven::No),
    }
}
