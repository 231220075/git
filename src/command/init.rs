use std::fs;
use std::path::PathBuf;
use std::convert::Into;
use std::env::current_dir;
use clap::{Parser, Subcommand, CommandFactory};

use crate::utils::{
    zlib::{
        decompress_file,
        decompress_file_as_bytes,
    },
    fs::obj_to_pathbuf,
    objtype::{
        ObjType,
        parse_meta,
        Obj,
    },
    blob::Blob,
    tree::Tree,
    commit::Commit,
};

use crate::{
    GitError,
    Result,
};
use super::SubCommand;

use nom::{
    bytes::complete::{tag, take, take_until},
    number::complete::be_u32,
    IResult,
};


#[derive(Parser, Debug)]
#[command(name = "init", about = "Create an empty Git repository or reinitialize an existing one")]
pub struct Init {
    #[arg(help = "directory to be initialized")]
    pub dir: Option<String>
}

impl Init {
    pub fn from_args(args: impl Iterator<Item = String>) -> Result<Box<dyn SubCommand>> {
        Ok(Box::new(Init::try_parse_from(args)?))
    }
}

impl SubCommand for Init {
    fn run(&self, _: Result<PathBuf>) -> Result<i32> {
        let curr_path = current_dir()?;
        let gitdir = if self.dir.is_some() {
            curr_path.join(self.dir.clone().unwrap())
        }
        else {
            curr_path
        }.join(".git");
        if gitdir.exists() {
            return Err(GitError::invalid_command(format!("{} directory already exists", gitdir.display())));
        }

        let refs = gitdir.join("refs");
        fs::create_dir_all(&refs)
            .map_err(|_| GitError::invalid_command(format!("Failed to create {} directory", refs.display())))?;
        fs::create_dir_all(refs.join("heads"))?;
        fs::create_dir_all(refs.join("tags"))?;
        fs::create_dir_all(refs.join("remote"))?;

        let objects = gitdir.join("objects");
        fs::create_dir_all(&objects)
            .map_err(|_| GitError::invalid_command(format!("Failed to create {} directory", objects.display())))?;
        fs::create_dir_all(objects.join("info"))?;
        fs::create_dir_all(objects.join("pack"))?;

        std::fs::write( gitdir.join("HEAD"), b"ref: refs/heads/master")
            .map_err(|_| GitError::invalid_command(format!("Failed to create {} file", gitdir.join("HEAD").display())))?;
        Ok(0)
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use crate::utils::{
        test::{
            shell_spawn,
            setup_test_git_dir,
            mktemp_in,
            tempdir,
        },
    };

    #[test]
    fn test_basic() -> Result<()> {
        let temp_dir = tempdir()?;
        let _ = shell_spawn(&["cargo", "run", "--quiet", "--", "init", temp_dir.path().to_str().unwrap()])?;
        let _ = shell_spawn(&["git", "-C", temp_dir.path().to_str().unwrap(), "status"])?;
        Ok(())
    }

    #[test]
    fn test_init_anthoer_repo() {
        let temp = setup_test_git_dir();

        let res = shell_spawn(&["cargo", "run", "--quiet", "--", "init", &temp.path().display().to_string()]);
        assert!(res.is_err());
    }
}
