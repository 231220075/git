use std::path::{PathBuf};
use clap::{Parser, Subcommand};
use std::fs;

use crate::{
    GitError,
    Result,
    utils::refs::{read_head_ref, read_ref_commit, write_ref_commit},
};

use super::SubCommand;

#[derive(Parser, Debug)]
#[command(name = "branch", about = "列出或创建分支")]
pub struct Branch {

    #[arg(short = 'd', long = "delete", help = "删除分支")]
    delete: bool,
    /// 新分支名（如果不指定则列出所有分支）
    branch_name: Option<String>,
}

impl Branch {
    pub fn from_args(args: impl Iterator<Item = String>) -> Result<Box<dyn SubCommand>> {
        Ok(Box::new(Branch::try_parse_from(args)?))
    }
}

impl SubCommand for Branch {
    fn run(&self, gitdir: Result<PathBuf>) -> Result<i32> {
        let gitdir = gitdir?;
        let heads_dir = gitdir.join("refs/heads");
        if self.delete {
            if let Some(ref branch_name) = self.branch_name {
                let branch_path = heads_dir.join(branch_name);
                if branch_path.exists() {
                    let current_ref = read_head_ref(&gitdir)?;
                    if format!("refs/heads/{}", branch_name) == current_ref {
                        return Err(GitError::invalid_command(format!("can not remove current ref '{}'", branch_name)));
                    }
                    fs::remove_file(&branch_path)
                        .map_err(|_| GitError::failed_to_write_file(&branch_path.to_string_lossy()))?;
                    //println!("Branch '{}' deleted", branch_name);
                } else {
                    //println!("Branch '{}' does not exist", branch_name);
                }
            } else {
                return Err(GitError::invalid_command("no file to remove".to_string()));
            }
        } else if let Some(ref branch_name) = self.branch_name {
            let head_ref = read_head_ref(&gitdir)?;
            let commit_hash = read_ref_commit(&gitdir, &head_ref)?;
            let new_branch = heads_dir.join(branch_name);
            if new_branch.exists() {
                return Err(GitError::invalid_command(format!("branch '{}' already exist", branch_name)));
            }
            fs::write(&new_branch, format!("{}\n", commit_hash))
                .map_err(|_| GitError::failed_to_write_file(&new_branch.to_string_lossy()))?;
            //println!("Branch '{}' created at {}", branch_name, commit_hash);
        } else {
            let current_ref = read_head_ref(&gitdir)?;
            for entry in fs::read_dir(&heads_dir)? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().to_string();
                if format!("refs/heads/{}", name) == current_ref {
                    // println!("* {}", name);
                } else {
                    // println!("  {}", name);
                }
            }
        }

        Ok(0)
    }
}
