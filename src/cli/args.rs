use crate::{
    GitError,
    Result,
};
use crate::command::{Init, Add, Rm, Commit, CatFile, SubCommand};


pub fn get_args(mut raw_args: impl Iterator<Item = String>) -> Result<Box<dyn SubCommand>> {
    let mut raw_args = raw_args.peekable();
    let command = raw_args.peek()
        .ok_or(GitError::no_subcommand())?;

    match command.as_str() {
        "cat-file" => CatFile::from_args(raw_args),
        "commit" => Commit::from_args(raw_args),
        "init"   => Init::from_args(raw_args),
        "add"    => Add::from_args(raw_args),
        "rm"     => Rm::from_args(raw_args),
        _        => Err(GitError::invalid_command())
    }
}


#[cfg(test)]
mod test {
    use super::*;

    fn to_strings<'a>(args: &[&str]) -> impl Iterator<Item = String> {
        args.into_iter().map(|x|String::from(*x))
    }

    #[test]
    fn test_init() {
        let args = to_strings(&["init"]);
        let command = get_args(args);
        assert!(command.is_ok());
        assert_eq!(format!("{:?}", command.unwrap()), format!("{:?}", Init{}));

        let args = to_strings(&["init", "-V", "foo:bar"]);
        let command = get_args(args);
        assert!(command.is_ok());

        assert_eq!(format!("{:?}", command.unwrap()), format!("{:?}", Init{}));

        let args = to_strings(&["init", "--", "aaaaaaaaaaaaaaaaaaaaaaaaaaaabbbbbbbbbbbbb"]);
        let command = get_args(args);
        assert!(command.is_ok());

        assert_eq!(format!("{:?}", command.unwrap()), format!("{:?}", Init{}));
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
        let command = get_args(args);
        assert!(command.is_err());

        File::create("add_tmp1");
        File::create("add_tmp2");
        File::create("add_tmp3");
        let args = to_strings(&["add", ".", "add_tmp1", "add_tmp2", "add_tmp3"]);
        let command = get_args(args);
        assert!(command.is_ok());
        remove_file("add_tmp1");
        remove_file("add_tmp2");
        remove_file("add_tmp3");
    }


    #[test]
    fn test_rm() {
        let args = to_strings(&["rm", "-n", "."]);
        let command = get_args(args);
        assert!(command.is_ok());

        let args = to_strings(&["rm", "-n", "."]);
        let command = get_args(args);
        assert!(command.is_ok());

        let args = to_strings(&["rm", "-n", ".no_exist_s"]);
        let command = get_args(args);
        assert!(command.is_err());

        File::create("rm_tmp1");
        File::create("rm_tmp2");
        File::create("rm_tmp3");
        let args = to_strings(&["rm", ".", "rm_tmp1", "rm_tmp2", "rm_tmp3"]);
        let command = get_args(args);
        assert!(command.is_ok());
        remove_file("rm_tmp1");
        remove_file("rm_tmp2");
        remove_file("rm_tmp3");
    }
}
