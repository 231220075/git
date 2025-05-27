use std::path::PathBuf;
use std::convert::Into;
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
#[command(name = "cat-file", about = "Provide contents or details of repository objects")]
pub struct CatFile {

    #[arg(required = true, short = 'p', group = "option", help = "pretty-print <object> content")]
    print: bool,

    #[arg(required = true, short = 'e', group = "option", help = "check if <object> exists")]
    check_exist: bool,

    #[arg(short = 't', group = "option", help = "show object type (one of 'blob', 'tree', 'commit', 'tag', ...)")]
    show_type: bool,

    #[arg(required = true, value_parser = obj_to_pathbuf)]
    objpath: PathBuf,
}

impl CatFile {
    pub fn from_args(args: impl Iterator<Item = String>) -> Result<Box<dyn SubCommand>> {
        Ok(Box::new(CatFile::try_parse_from(args)?))
    }

    pub fn cat(&self, path: PathBuf) -> Result<()> {
        let obj: Obj = decompress_file_as_bytes(&path)?.try_into()?;
        print!("{}", obj);
        Ok(())
    }

    pub fn cat_type(&self, path: PathBuf) -> Result<()> {
        let bytes = decompress_file_as_bytes(&path)?;
        let (_, (t, _)) = parse_meta(&bytes).map_err(|x|x.to_string()).map_err(GitError::invalid_obj)?;
        println!("{}", String::from_utf8(t.to_vec()).map_err(|x|x.to_string()).map_err(GitError::invalid_obj)?);
        Ok(())
    }
}


impl SubCommand for CatFile {
    fn run(&self, gitdir: Result<PathBuf>) -> Result<i32> {
        let mut gitdir = gitdir?;
        gitdir.push(&self.objpath);
        if !gitdir.exists()
        {
            if self.check_exist {
                Ok((!gitdir.exists()) as i32)
            }
            else {
                Err(GitError::file_notfound(format!("{} 不存在", gitdir.to_str().unwrap())))
            }
        }
        else if self.print {
            self.cat(gitdir)?;
            Ok(0)
        }
        else if self.show_type {
            self.cat_type(gitdir)?;
            Ok(0)
        }
        else {
            let mut cmd = CatFile::command(); // 获取底层的 Command 对象
            let _ = cmd.print_help();     // 打印帮助信息
            Ok(0)
        }
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
        },
    };

    #[test]
    fn test_blob() {
        let temp = setup_test_git_dir();
        let temp_path = temp.path();
        let temp_path_str = temp_path.to_str().unwrap();

        let file1 = mktemp_in(&temp).unwrap();
        let file1_str = file1.to_str().unwrap();

        let _ = shell_spawn(&["ls", "-lahR", temp_path_str]).unwrap();

        let _ = shell_spawn(&["git", "-C", temp_path_str, "add", &file1_str]).unwrap();
        let hash = shell_spawn(&["git", "-C", temp_path_str, "hash-object", file1_str]).unwrap();
        let hash = hash.strip_suffix("\n").unwrap();

        let origin = shell_spawn(&["git", "-C", temp_path_str, "cat-file", "-p", &hash]).unwrap();
        let real = shell_spawn(&["cargo", "run", "--quiet", "--", "-C", temp_path_str, "cat-file", "-p", &hash]).unwrap();
        assert_eq!(origin, real);
    }

    #[test]
    fn test_tree() {

        let temp = setup_test_git_dir();
        let temp_path = temp.path();
        let temp_path_str = temp_path.to_str().unwrap();

        let file1 = mktemp_in(&temp).unwrap();
        let file1_str = file1.to_str().unwrap();
        let file2 = mktemp_in(&temp).unwrap();
        let file2_str = file2.to_str().unwrap();

        let _ = shell_spawn(&["git", "-C", temp_path_str, "update-index", "--add", &file1_str, &file2_str]).unwrap();
        let hash = shell_spawn(&["git", "-C", temp_path_str, "write-tree"]).unwrap();
        let hash = hash.strip_suffix("\n").unwrap();

        let origin = shell_spawn(&["git", "-C", temp_path_str, "cat-file", "-p", &hash]).unwrap();
        println!("origin = {}", origin);
        let real = shell_spawn(&["cargo", "run", "--quiet", "--", "-C", temp_path_str, "cat-file", "-p", &hash]).unwrap();
        assert_eq!(origin, real);
    }

    #[test]
    fn test_commit() {
        let temp = setup_test_git_dir();
        let temp_path = temp.path();
        let temp_path_str = temp_path.to_str().unwrap();

        let file1 = mktemp_in(&temp).unwrap();
        let file1_str = file1.to_str().unwrap();
        let file2 = mktemp_in(&temp).unwrap();
        let file2_str = file2.to_str().unwrap();
        let _ = shell_spawn(&["git", "-C", temp_path_str, "add", &file1_str, &file2_str]).unwrap();
        let _ = shell_spawn(&["git", "-C", temp_path_str, "commit", "-m", "commit-message"]).unwrap();
        let hash = shell_spawn(&["git", "-C", temp_path_str, "rev-parse", "HEAD"]).unwrap();
        let hash = hash.strip_suffix("\n").unwrap();

        let origin = shell_spawn(&["git", "-C", temp_path_str, "cat-file", "-p", &hash]).unwrap();
        println!("{}", origin);
        let real = shell_spawn(&["cargo", "run", "--quiet", "--", "-C", temp_path_str, "cat-file", "-p", &hash]).unwrap();
        println!("{}", real);
        assert_eq!(origin, real);
    }
}
