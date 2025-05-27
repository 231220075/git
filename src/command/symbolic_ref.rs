use std::path::PathBuf;
use clap::{Parser, Subcommand};
use crate::{
    GitError,
    Result,
};
use crate::utils::refs::{read_head_ref, write_head_ref};
use super::SubCommand;

#[derive(Parser, Debug)]
#[command(name = "symbolic-ref", about = "读取或设置符号引用（如HEAD）")]
pub struct SymbolicRef {
    /// 要操作的引用（如 HEAD），默认是 HEAD
    #[arg(default_value = "HEAD")]
    ref_name: String,

    /// 设置引用为指定目标（如 refs/heads/main）
    #[arg(required = false)]
    new_target: Option<String>,
}

impl SymbolicRef {
    pub fn from_args(args: impl Iterator<Item = String>) -> Result<Box<dyn SubCommand>> {
        let symbolic_ref = SymbolicRef::try_parse_from(args)?;
        Ok(Box::new(symbolic_ref))
    }
}

impl SubCommand for SymbolicRef {
    fn run(&self, gitdir: Result<PathBuf>) -> Result<i32> {
        let gitdir = gitdir?;
        if let Some(ref target) = self.new_target {
            if self.ref_name != "HEAD" {
                return Err(Box::new(GitError::InvalidCommand("只支持设置 HEAD 的符号引用".to_string())));
            }
            write_head_ref(&gitdir, target)?;
            //println!("Updated HEAD to {}", target);
        } else {
            let ref_value = if self.ref_name == "HEAD" {
                read_head_ref(&gitdir)?
            } else {
                return Err(Box::new(GitError::InvalidCommand("只支持读取 HEAD 的符号引用".to_string())));
            };
            //println!("{}", ref_value);
        }
        Ok(0)
    }
}