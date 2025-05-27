use itertools::Update;

use std::path::PathBuf;
use clap::{Parser, Subcommand, CommandFactory, ValueHint};

use crate::{
    utils::fs::{
        get_git_dir,
        search_git_dir,
        to_pathbuf,
    },
    command::{
        Init, Add, Rm, Commit, Branch, Checkout,
        CatFile, SubCommand, HashObject,
        UpdateIndex, CommitTree, ReadTree, WriteTree,
        Merge,
    },
    GitError,
    Result,
};


#[derive(Parser, Debug)]
#[command(name = "git", about = "git commandline")]
pub struct Git {

    #[arg(short = 'C', value_hint = ValueHint::DirPath, help = "Run as if git was started in <path> instead of the current working directory.")]
    change_dir: Option<PathBuf>,

    #[arg(required = true, allow_hyphen_values = true)]
    subcommands: Vec<String>,
}


impl Git {
    pub fn from_args(args: impl Iterator<Item = String>) -> Result<Self> {
        Ok(Self::try_parse_from(args)?)
    }

    pub fn execute(&mut self) -> Result<i32> {
        get_args(self.subcommands.clone().into_iter())
            .and_then(|cmd| {
                if self.change_dir.is_some() {
                    cmd.run(search_git_dir(self.change_dir.take().unwrap()))
                }
                else {
                    cmd.run(get_git_dir())
                }
            })
    }
}

pub fn get_args(raw_args: impl Iterator<Item=String>) -> Result<Box<dyn SubCommand>> {
    let mut raw_args = raw_args.into_iter().peekable();
    let command = raw_args.peek()
        .ok_or(GitError::no_subcommand())?;

    match command.as_str() {
        "hash-object" => HashObject::from_args(raw_args),
        "cat-file" => CatFile::from_args(raw_args),
        "commit" => Commit::from_args(raw_args),
        "merge" => Merge::from_args(raw_args),
        "init"   => Init::from_args(raw_args),
        "add"    => Add::from_args(raw_args),
        "rm"     => Rm::from_args(raw_args),
        "branch" => Branch::from_args(raw_args),
        "checkout" => Checkout::from_args(raw_args),
        "update-index" => UpdateIndex::from_args(raw_args),
        "write-tree" => WriteTree::from_args(raw_args),
        "commit-tree" => CommitTree::from_args(raw_args),
        "read-tree" => ReadTree::from_args(raw_args),
        unkown => Err(GitError::invalid_command(unkown.to_string()))
    }
}



#[cfg(test)]
mod test {
    use super::*;

    fn to_strings(args: &[&str]) -> impl Iterator<Item = String> {
        args.iter().map(|&s|String::from(s))
    }

    #[test]
    fn test_init() {
        let args = to_strings(&["init"]);
        let command = get_args(args);
        assert!(command.is_ok());
        assert_eq!(format!("{:?}", command.unwrap()), format!("{:?}", Init{dir: None}));
    }

    #[test]
    fn test_commit() {
        let args = to_strings(&["commit"]);
        let command = get_args(args);
        assert!(command.is_err());

        let args = to_strings(&["commit", "-v", "-m", "message", "-aasdvas"]);
        let command = get_args(args);
        assert!(command.is_err());


        let args = to_strings(&["commit", "-m", "messages"]);
        let command = get_args(args);
        assert!(command.is_ok());
        assert_eq!(format!("{:?}", command.unwrap()), format!("{:?}", Commit { message: Some("messages".to_string()), all: false }));

        let args = to_strings(&["commit", "-m", "messages", "-a"]);
        let command = get_args(args);
        assert!(command.is_ok());
        assert_eq!(format!("{:?}", command.unwrap()), format!("{:?}", Commit { message: Some("messages".to_string()), all: true }));

        let args = to_strings(&["commit", "--message", "messages", "--all"]);
        let command = get_args(args);
        assert!(command.is_ok());
        assert_eq!(format!("{:?}", command.unwrap()), format!("{:?}", Commit { message: Some("messages".to_string()), all: true }));
    }

    use std::fs::{
        File,
        remove_file,
    };

    #[test]
    fn test_add() {
        let args = to_strings(&["add", "-n", "."]);
        let command = get_args(args);
        assert!(command.is_ok());

        let args = to_strings(&["add", "-n", "."]);
        let command = get_args(args);
        assert!(command.is_ok());

        let args = to_strings(&["add", "-n", ".no_exist_s"]);
        let command = get_args(args).unwrap();
        let a = command.run(Ok(PathBuf::from("/tmp")));
        assert!(a.is_err());

        File::create("add_tmp1").unwrap();
        File::create("add_tmp2").unwrap();
        File::create("add_tmp3").unwrap();
        let args = to_strings(&["add", ".", "add_tmp1", "add_tmp2", "add_tmp3"]);
        let command = get_args(args);
        remove_file("add_tmp1").unwrap();
        remove_file("add_tmp2").unwrap();
        remove_file("add_tmp3").unwrap();
        assert!(command.is_ok());
    }


    #[test]
    fn test_rm() {
        let args = to_strings(&["rm", "--cached", "-n", "."]);
        let command = get_args(args);
        assert!(command.is_ok());

        let args = to_strings(&["rm", "--cached", "-n", "."]);
        let command = get_args(args);
        assert!(command.is_ok());

        let args = to_strings(&["rm", "--cached", "-n", ".no_exist_s"]);
        let command = get_args(args).unwrap();
        let a = command.run(Ok(PathBuf::from("/tmp")));
        assert!(a.is_err());

        File::create("rm_tmp1").unwrap();
        File::create("rm_tmp2").unwrap();
        File::create("rm_tmp3").unwrap();
        let args = to_strings(&["rm", ".", "rm_tmp1", "rm_tmp2", "rm_tmp3"]);
        let command = get_args(args);
        assert!(command.is_ok());
        remove_file("rm_tmp1").unwrap();
        remove_file("rm_tmp2").unwrap();
        remove_file("rm_tmp3").unwrap();
    }
}
