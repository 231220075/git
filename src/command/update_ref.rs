use std::path::{PathBuf,Path};
use clap::{Parser, Subcommand};
use crate::utils::index;
use crate::utils::zlib::{compress_object, decompress_file_bytes};
use crate::{
    GitError,
    Result,
};
use crate::utils::{
    fs::read_file_as_bytes,
    hash::hash_object,
    index::{Index, IndexEntry},
    tree::{
        Tree,
        FileMode,
    },
};
use super::SubCommand;
#[derive(Parser, Debug)]
#[command(name = "update-ref", about = "update the ref file")]
pub struct UpdateRef {
    #[arg(required = true, help = "Prefix to add to all paths in the tree")]
    pub ref_path: String,

    #[arg(required = true, help = "tree hash")]
    pub commit_hash: String,
}

impl UpdateRef {
    pub fn from_args(args: impl Iterator<Item = String>) -> Result<Box<dyn SubCommand>> {
        let update_ref = UpdateRef::try_parse_from(args)?;
        Ok(Box::new(update_ref))
    }
}

impl SubCommand for UpdateRef {
    fn run(&self, gitdir: Result<PathBuf>) -> Result<i32> {
        let gitdir = gitdir?;
        let ref_path = gitdir.join(&self.ref_path);

        std::fs::write(&ref_path, format!("{}\n", self.commit_hash))
            .map_err(|_| GitError::failed_to_write_file(&ref_path.to_string_lossy()))?;

        //println!("Updated ref {} to {}", self.ref_path, self.commit_hash);
        Ok(0)
    }
}
