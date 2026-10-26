//! The subcommands, one module each.

pub mod app;
pub mod convert;
pub mod get;
pub mod gitignore;
pub mod join;
pub mod list;
pub mod mv;
pub mod new;
pub mod overwrite;
pub mod remove;
pub mod rename_device;
pub mod rename_folder;
pub mod share;
pub mod sync;
pub mod unshare;

use anyhow::Result;
use clap::Subcommand;
use itertools::Itertools;

#[derive(Subcommand, Debug, PartialEq)]
pub enum Command {
    #[command(visible_aliases = ["ui", "run"])]
    App(app::Args),
    #[command(visible_alias = "init")]
    Join(join::Args),
    Convert(convert::Args),
    List(list::Args),
    New(new::Args),
    Get(get::Args),
    Remove(remove::Args),
    Share(share::Args),
    Unshare(unshare::Args),
    Sync(sync::Args),
    RenameDevice(rename_device::Args),
    RenameFolder(rename_folder::Args),
    Mv(mv::Args),
    Overwrite(overwrite::Args),
    Gitignore(gitignore::Args),
}

pub fn run(command: Command) -> Result<()> {
    match command {
        Command::App(args) => app::run(args),
        Command::Join(args) => join::run(args),
        Command::Convert(args) => convert::run(args),
        Command::List(args) => list::run(args),
        Command::New(args) => new::run(args),
        Command::Get(args) => get::run(args),
        Command::Remove(args) => remove::run(args),
        Command::Share(args) => share::run(args),
        Command::Unshare(args) => unshare::run(args),
        Command::Sync(args) => sync::run(args),
        Command::RenameDevice(args) => rename_device::run(args),
        Command::RenameFolder(args) => rename_folder::run(args),
        Command::Mv(args) => mv::run(args),
        Command::Overwrite(args) => overwrite::run(args),
        Command::Gitignore(args) => gitignore::run(args),
    }
}

/// A command's arguments, as the words of the command line running it.
pub trait Arguments {
    fn arguments(&self) -> Vec<String>;
}

impl Command {
    /// The `synchub` command line running it, quoted for a shell - what the
    /// app logs for what its actions did.
    pub fn command_line(&self) -> String {
        std::iter::once("synchub".to_string())
            .chain(self.words().iter().map(|word| quote(word)))
            .join(" ")
    }

    /// The words after `synchub`: the command's name, then its arguments.
    fn words(&self) -> Vec<String> {
        let (name, arguments) = match self {
            Command::App(args) => ("app", args.arguments()),
            Command::Join(args) => ("join", args.arguments()),
            Command::Convert(args) => ("convert", args.arguments()),
            Command::List(args) => ("list", args.arguments()),
            Command::New(args) => ("new", args.arguments()),
            Command::Get(args) => ("get", args.arguments()),
            Command::Remove(args) => ("remove", args.arguments()),
            Command::Share(args) => ("share", args.arguments()),
            Command::Unshare(args) => ("unshare", args.arguments()),
            Command::Sync(args) => ("sync", args.arguments()),
            Command::RenameDevice(args) => ("rename-device", args.arguments()),
            Command::RenameFolder(args) => ("rename-folder", args.arguments()),
            Command::Mv(args) => ("mv", args.arguments()),
            Command::Overwrite(args) => ("overwrite", args.arguments()),
            Command::Gitignore(args) => ("gitignore", args.arguments()),
        };
        std::iter::once(name.to_string()).chain(arguments).collect()
    }
}

/// `word` quoted for a shell, if it needs to be.
fn quote(word: &str) -> String {
    let plain = |c: char| c.is_ascii_alphanumeric() || "-_./=:@+,%".contains(c);
    // A leading ~ is left for the shell to expand.
    let (tilde, rest) = match word.strip_prefix("~/") {
        Some(rest) => ("~/", rest),
        None if word == "~" => return word.to_string(),
        None => ("", word),
    };
    if !rest.is_empty() && rest.chars().all(plain) {
        format!("{tilde}{rest}")
    } else {
        format!("{tilde}'{}'", rest.replace('\'', r"'\''"))
    }
}

/// A word for an optional flag: `flag` if `on`.
fn flag(on: bool, flag: &str) -> Option<String> {
    on.then(|| flag.to_string())
}

/// The words for an optional value: `option value`, if there is one.
fn option(option: &str, value: Option<impl ToString>) -> Vec<String> {
    value
        .map(|value| vec![option.to_string(), value.to_string()])
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct Cli {
        #[command(subcommand)]
        command: Command,
    }

    /// Asserts that `command`'s words parse back into `command` - with
    /// `extra` words after them.
    fn round_trips(command: Command, extra: &[&str]) {
        let line = std::iter::once("synchub".to_string())
            .chain(command.words())
            .chain(extra.iter().map(|word| word.to_string()));
        let parsed = Cli::try_parse_from(line)
            .unwrap_or_else(|err| panic!("synchub doesn't take it: {err}"))
            .command;
        assert_eq!(parsed, command);
    }

    #[test]
    fn command_lines_are_quoted_for_a_shell() {
        let command = Command::Mv(mv::Args {
            source: "~/my notes".into(),
            dest: "~".into(),
            force: false,
        });
        assert_eq!(command.command_line(), "synchub mv ~/'my notes' ~");
        let command = Command::New(new::Args {
            path: "it's".into(),
            label: Some(String::new()),
            devices: Vec::new(),
        });
        assert_eq!(command.command_line(), r"synchub new 'it'\''s' --label ''");
    }

    #[test]
    fn command_lines_parse_back_into_their_arguments() {
        round_trips(Command::App(app::Args {}), &[]);
        round_trips(Command::Convert(convert::Args {}), &[]);
        round_trips(Command::List(list::Args {}), &[]);
        round_trips(Command::Sync(sync::Args {}), &[]);
        round_trips(
            Command::Overwrite(overwrite::Args {
                local_dir: "~/the copy".into(),
                folder_id: "abcde-fghij".parse().unwrap(),
                force: true,
            }),
            &[],
        );
        round_trips(
            Command::Remove(remove::Args {
                folder: "abcde-fghij".into(),
                delete_files: true,
            }),
            &[],
        );
        round_trips(
            Command::Share(share::Args {
                folder: "Notes".into(),
                device: "laptop".into(),
            }),
            &[],
        );
        round_trips(
            Command::Unshare(unshare::Args {
                folder: "Notes".into(),
                device: "laptop".into(),
            }),
            &[],
        );
        round_trips(
            Command::Get(get::Args {
                folder: "abcde-fghij".into(),
                path: Some(PathBuf::from("~/in here")),
            }),
            &[],
        );
        round_trips(
            Command::Gitignore(gitignore::Args {
                path: "~/projects".into(),
            }),
            &[],
        );
        round_trips(
            Command::RenameDevice(rename_device::Args {
                device: "LAPTOP1".into(),
                name: "the laptop".into(),
            }),
            &[],
        );
        round_trips(
            Command::RenameFolder(rename_folder::Args {
                folder: "abcde-fghij".into(),
                label: "Notes".into(),
            }),
            &[],
        );
        round_trips(
            Command::Mv(mv::Args {
                source: "~/notes".into(),
                dest: "~/work/notes".into(),
                force: true,
            }),
            &[],
        );
        round_trips(
            Command::New(new::Args {
                path: "~/my notes".into(),
                label: Some("Notes".into()),
                devices: vec!["laptop".into(), "phone".into()],
            }),
            &[],
        );
        // The API key isn't in it: it's taken from SYNCHUB_HUB_API_KEY.
        round_trips(
            Command::Join(join::Args {
                url: "https://hub:8384".into(),
                api_key: "KEY".into(),
                device_id: Some(
                    "NLHZ5AC-IMOKBMU-4FWXEPU-CXFUK2Y-G5VFCRK-XGZBJ33-TV7VMVP-VPDVHAP"
                        .parse()
                        .unwrap(),
                ),
                yes: true,
                default_folders_dir: Some("~/Sync".into()),
            }),
            &["KEY"],
        );
    }
}
