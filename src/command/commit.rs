use std::path::PathBuf;
use clap::{Parser, Subcommand, Command};
use std::io::{self, Write, Cursor};
use std::process::Command as ProcessCommand;

use crate::{
    GitError,
    Result,
    command::{
        WriteTree, CommitTree, UpdateRef
    },
    utils:: {
        commit,
        tree::Tree,
        index::Index,
        fs::write_object,
        refs::{
            read_head_ref, read_ref_commit
        },
        hash::hash_object,
    },
};
use super::SubCommand;

#[derive(Parser, Debug)]
#[command(name = "commit", about = "记录对存储库的更改")]
pub struct Commit {
    #[arg(short, long, value_name="MESSAGE", help = "commit message")]
    pub message: Option<String>,

    #[arg(short, long, help = "commit all changed files")]
    pub all: bool
}

impl Commit {
    pub fn from_args(args: impl Iterator<Item = String>) -> Result<Box<dyn SubCommand>> {
        let cli = Commit::try_parse_from(args)?;
        cli.message
            .ok_or_else(||GitError::invalid_command("todo, 在这里调用$EDITOR".to_string()))
            .map(|message| Box::new(Commit {
                message: Some(message),
                all: cli.all,
            }) as Box<dyn SubCommand>)
    }

}

impl SubCommand for Commit {
    fn run(&self, gitdir: Result<PathBuf>) -> Result<i32> {
        let gitdir = gitdir?;
        let index_path = gitdir.join("index");

        let index = Index::new();
        let tree: Tree = index.read_from_file(&index_path)?.into();
        let tree_hash = write_object::<Tree>(gitdir.clone(), tree.into())?;

        let head_ref = read_head_ref(&gitdir)?;
        let parent_commit = read_ref_commit(&gitdir, &head_ref).ok();

        let commit = commit::Commit {
            tree_hash,
            parent_hash: if parent_commit.is_none() {vec![]} else { vec![parent_commit.unwrap()] },
            author: "Default Author <139881912@163.com> 1748165415 +0800".into(),
            committer: "commiter Author <139881912@163.com> 1748165415 +0800".into(),
            message: self.message.clone().unwrap(),
        };

        let commit_hash = write_object::<commit::Commit>(gitdir.clone(), commit.into())?;

        let update_ref = UpdateRef {
            ref_path: head_ref,
            commit_hash: commit_hash.clone()
        };
        update_ref.run(Ok(gitdir))?;

        println!("{}", commit_hash);
        Ok(0)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::utils::test::{
        shell_spawn,
        setup_test_git_dir,
        mktemp_in,
        touch_file_in,
        cmd_seq,
        tempdir,
        cp_dir,
        run_both,
        ArgsList,
    };

    #[test]
    fn test_simple() {
        let temp1 = setup_test_git_dir();
        let temp_path1 = temp1.path();
        let temp_path_str1 = temp_path1.to_str().unwrap();

        let temp2 = tempdir().unwrap();
        let temp_path2 = temp2.path();
        let temp_path_str2 = temp_path2.to_str().unwrap();

        let file1 = mktemp_in(&temp1).unwrap();
        let file1_str = file1.file_name().unwrap();
        let file1_str = file1_str.to_str().unwrap();

        let file2 = mktemp_in(&temp1).unwrap();
        let file2_str = file2.file_name().unwrap().to_str();
        let file2_str = file2_str.unwrap();

        let _ = cp_dir(temp_path1, temp_path2).unwrap();

        let cmds: ArgsList = &[
            (&["add", file1_str, file2_str], true),
            (&["commit", "-m", "commit message"], true),
        ];
        let git = &["git", "-C", temp_path_str1];
        let cargo = &["cargo", "run", "--quiet", "--", "-C", temp_path_str2];
        let _ = run_both(cmds, git, cargo).unwrap();

        println!("{}", shell_spawn(&["ls", "-lahR", temp_path_str2]).unwrap());
        println!("{}", shell_spawn(&["cat", PathBuf::from(temp_path_str2).join(".git").join("HEAD").to_str().unwrap()]).unwrap());

        let origin = shell_spawn(&["git", "-C", temp_path_str1, "symbolic-ref", "HEAD"]).unwrap();
        let real = shell_spawn(&["git", "-C", temp_path_str2, "symbolic-ref", "HEAD"]).unwrap();
        assert_eq!(origin, real);
    }


    #[test]
    fn test_ppt_add_commit() -> Result<()> {
        let temp_dir = tempdir()?;
        let project_root = env!("CARGO_MANIFEST_DIR");
        std::env::set_current_dir(project_root).unwrap();
        let curr_dir = std::env::current_dir().unwrap();

        println!("{}", shell_spawn(&["mkdir", temp_dir.path().join("tests").to_str().unwrap()])?);
        println!("{}", shell_spawn(&["cp", curr_dir.join("target/debug/git").to_str().unwrap(), temp_dir.path().join("tests/rust-git").to_str().unwrap()])?);
        println!("{}", shell_spawn(&["cp", curr_dir.join("tests/test2.png").to_str().unwrap(), temp_dir.path().join("tests/test.png").to_str().unwrap()])?);
        println!("{}", shell_spawn(&["cp", curr_dir.join("tests/test2.png").to_str().unwrap(), temp_dir.path().join("tests/git.png").to_str().unwrap()])?);
        println!("{}", shell_spawn(&["chmod", "a+x", temp_dir.path().join("tests").join("rust-git").to_str().unwrap()])?);

        std::env::set_current_dir(&temp_dir)?;
        println!("output = {}", shell_spawn(&[curr_dir.join("tests/test_add_commit").to_str().unwrap()])?);
        Ok(())
    }
}
