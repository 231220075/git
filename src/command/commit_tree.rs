use std::env;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use clap::{Parser, Subcommand};
use crate::utils::{
    zlib::compress_object,
    hash::hash_object,
    commit::Commit,
    fs::write_object,
};
use crate::{
    GitError,
    Result,
};
use super::SubCommand;

#[derive(Parser, Debug)]
#[command(name = "commit-tree", about = "Create a commit object from a tree object")]
pub struct CommitTree {
    #[arg(required = true, help = "The tree object hash")]
    pub tree_hash: String,

    #[arg(short = 'm', required = true, help = "The commit message")]
    pub message: String,

    #[arg(short = 'p', help = "The parent commit hash")]
    pub pcommit: Option<String>,
}

impl CommitTree {
    pub fn from_args(args: impl Iterator<Item = String>) -> Result<Box<dyn SubCommand>> {
        Ok(Box::new(CommitTree::try_parse_from(args)?))
    }

    pub fn get_author_info() -> (String, String) {
        let author_name = env::var("GIT_AUTHOR_NAME").unwrap_or_else(|_| "Default Name".to_string());
        let author_email = env::var("GIT_AUTHOR_EMAIL").unwrap_or_else(|_| "default_email@example.com".to_string());
        (author_name, author_email)
    }

    pub fn build_commit_content(&self) -> String {
        let (author_name, author_email) = Self::get_author_info();

        let mut content = format!("tree {}\n", self.tree_hash);

        if let Some(parent) = &self.pcommit {
            content.push_str(&format!("parent {}\n", parent));
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let timezone = "+0000"; 

        content.push_str(&format!(
            "author {} <{}> {} {}\n",
            author_name, author_email, timestamp, timezone
        ));
        content.push_str(&format!(
            "committer {} <{}> {} {}\n\n",
            author_name, author_email, timestamp, timezone
        ));

        content.push_str(&self.message);

        content
    }

    pub fn asshole(self, gitdir: PathBuf) -> Result<String> {
        let commit_content = self.build_commit_content();

        write_object::<Commit>(gitdir, commit_content.into_bytes())
    }
}

impl SubCommand for CommitTree {
    fn run(&self, gitdir: Result<PathBuf>) -> Result<i32> {
        let commit_content = self.build_commit_content();

        let commit_hash = write_object::<Commit>(gitdir?, commit_content.into_bytes())?;

        println!("{}", commit_hash);

        Ok(0)
    }
}






#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use crate::utils::test::{
        shell_spawn,
        setup_test_git_dir,
        mktemp_in,
    };

    #[test]
    fn test_build_commit_content() {
        let commit_tree = CommitTree {
            tree_hash: "d8329fc1cc938780ffdd9f94e0d364e0ea74f579".to_string(),
            message: "Initial commit".to_string(),
            pcommit: Some("8ea8033adc42a4148773457c1ad871d9e2f21d2e".to_string()),
        };

        let content = commit_tree.build_commit_content();

        assert!(content.contains("tree d8329fc1cc938780ffdd9f94e0d364e0ea74f579"));
        assert!(content.contains("parent 8ea8033adc42a4148773457c1ad871d9e2f21d2e"));
        assert!(content.contains("author Default Name <default_email@example.com>"));
        assert!(content.contains("committer Default Name <default_email@example.com>"));
        assert!(content.contains("Initial commit"));
    }

    #[test]
    fn test_write_commit_object() {
        use crate::utils::{
            fs::write_object,
            commit::Commit,
        };
        let temp_dir = setup_test_git_dir();
        //println!("{:?}", temp_dir);
        let git_dir = temp_dir.path().join(".git");

        // 设置当前工作目录
        std::env::set_current_dir(&temp_dir).unwrap();

        let commit_tree = CommitTree {
            tree_hash: "d8329fc1cc938780ffdd9f94e0d364e0ea74f579".to_string(),
            message: "Initial commit".to_string(),
            pcommit: None,
        };

        let content = commit_tree.build_commit_content();
        let commit_hash = write_object::<Commit>(git_dir.clone(), content.into_bytes()).unwrap();

        let object_path = git_dir
            .join("objects")
            .join(&commit_hash[0..2])
            .join(&commit_hash[2..]);

        //println!("Object path: {:?}", object_path); // 调试输出

        assert!(object_path.exists());

        let compressed_data = fs::read(object_path).unwrap();
        assert!(!compressed_data.is_empty());
    }

    #[test]
    fn test_with_git() {
        use super::super::CatFile;
        let temp_dir = setup_test_git_dir();
        let temp_dir = temp_dir.path().to_str().unwrap();
        let gitdir = PathBuf::from(temp_dir).join(".git");
        let _ = std::env::set_current_dir(temp_dir);

        let _ = mktemp_in(temp_dir);
        let _ = mktemp_in(temp_dir);
        let _ = shell_spawn(&["git", "-C", temp_dir, "add", ":/"]).unwrap();
        let tree_hash = shell_spawn(&["git", "-C", temp_dir, "write-tree"]).unwrap();

        let commit_tree = CommitTree::try_parse_from(&["commit-tree", &tree_hash, "-m", "test_with_git"]).unwrap();
        let content = commit_tree.build_commit_content();
        let commit_hash = write_object::<Commit>(gitdir.clone(), content.clone().into_bytes()).unwrap();


        let out = shell_spawn(&["git", "-C", temp_dir, "cat-file", "-p", &commit_hash]).unwrap();
        assert_eq!(content, out);
        //println!("{}", out);
    }
}
