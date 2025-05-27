use std::{
    path::{
        PathBuf,
        Path,
    },
    result,
    iter::once,
};
use clap::{Parser, Subcommand};
use itertools::Either;

use crate::{
    GitError,
    Result,
    utils::{
        objtype::ObjType,
        index::{
            Index,
            IndexEntry,
        },
        fs::{
            walk,
            write_object,
            read_file_as_bytes,
            add_object,
            calc_relative_path,
        },
        tree::FileMode,
        blob::Blob,
    },
};

use super::SubCommand;

fn output(input: &str) -> result::Result<PathBuf, String> {
    //println!("input = {}", input);
    Ok(PathBuf::from(input))
}

#[derive(Parser, Debug)]
#[command(name = "add", about = "将文件内容添加到索引中")]
pub struct Add {
    #[arg(short = 'n', long = "dry-run", help = "dry run", action = clap::ArgAction::SetTrue, required = false)]
    dry_run: bool,

    #[arg(required = true, num_args = 1.., value_parser=output)]
    paths: Vec<PathBuf>,
}

impl Add {
    pub fn from_args(args: impl Iterator<Item = String>) -> Result<Box<dyn SubCommand>> {
        Ok(Box::new(Add::try_parse_from(args)?))
    }

    fn walk_path(&self, project_root: PathBuf) -> Result<Vec<PathBuf>> {
        self.paths.clone().into_iter()
            .map(|p|project_root.join(p))
            .map(walk)
            .collect::<Result<Vec<_>>>()?.into_iter()
            .flatten()
            .filter(|x| !x.starts_with(project_root.join(".git")))
            .map(|p| calc_relative_path(&project_root, &p))
            .collect::<Result<Vec<_>>>()
    }
}

impl SubCommand for Add {
    fn run(&self, gitdir: Result<PathBuf>) -> Result<i32> {
        let gitdir = gitdir?;
        let index_file = gitdir.join("index");
        let project_root = gitdir.parent().expect("find git dir implementation fail");

        let mut index = Index::new();
        if index_file.exists() {
            // ugly
            index = index.read_from_file(&index_file)?;
        }

        //println!("index_file exists index = {:?}", index);

        let _ = self.walk_path(project_root.to_path_buf())?
            .into_iter()
            .map(|path| -> Result<()> {
                let path_string = path.display().to_string();
                if let Some(i) = index.entries.iter().position(|en|en.name == path_string) {
                    index.entries[i] = add_object::<Blob>(gitdir.clone(), path.clone())?
                }
                else {
                    index.add_entry(add_object::<Blob>(gitdir.clone(), path.clone())?);
                }
                Ok(())
            })
            .collect::<Result<Vec<_>>>()?;
        index.write_to_file(&index_file)?;
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
    fn test_basic() {
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
        ];
        let git = &["git", "-C", temp_path_str1];
        let cargo = &["cargo", "run", "--quiet", "--", "-C", temp_path_str2];
        let _ = run_both(cmds, git, cargo).unwrap();

        println!("{}", shell_spawn(&["ls", "-lahR", temp_path_str1]).unwrap());

        let origin = shell_spawn(&["git", "-C", temp_path_str1, "ls-files", "--stage", "|", "sort"]).unwrap();
        let real = shell_spawn(&["git", "-C", temp_path_str2, "ls-files", "--stage", "|", "sort"]).unwrap();
        assert_eq!(origin, real);
    }

    #[test]
    fn test_add_directory() {
        let temp1 = setup_test_git_dir();
        let temp_path1 = temp1.path();
        let temp_path_str1 = temp_path1.to_str().unwrap();

        let temp2 = tempdir().unwrap();
        let temp_path2 = temp2.path();
        let temp_path_str2 = temp_path2.to_str().unwrap();

        let file1 = touch_file_in(temp_path1.join("inner")).unwrap();
        let file1_str = file1.path().file_name().unwrap();
        let file1_str = file1_str.to_str().unwrap();

        let file2 = touch_file_in(temp_path1.join("inner").join("close")).unwrap();
        let file2_str = file2.path().file_name().unwrap();
        let file2_str = file2_str.to_str().unwrap();

        let _ = cp_dir(temp_path1, temp_path2).unwrap();

        let a = PathBuf::from("inner").join(file1_str);
        let b = PathBuf::from("inner/close").join(file2_str);
        let cmds: ArgsList = &[
            (&["add", a.to_str().unwrap(), b.to_str().unwrap()], true),
        ];
        let git = &["git", "-C", temp_path_str1];
        println!("{}", shell_spawn(&["ls", "-lahR", temp_path_str2]).unwrap());
        let cargo = &["cargo", "run", "--quiet", "--", "-C", temp_path_str2];
        let _ = run_both(cmds, git, cargo).unwrap();

        println!("{}", shell_spawn(&["ls", "-lahR", temp_path_str1]).unwrap());

        let origin = shell_spawn(&["git", "-C", temp_path_str1, "ls-files", "--stage", "|", "sort"]).unwrap();
        let real = shell_spawn(&["git", "-C", temp_path_str2, "ls-files", "--stage", "|", "sort"]).unwrap();
        assert_eq!(origin, real);
    }

    #[test]
    fn test_add_twice() {
        let temp1 = setup_test_git_dir();
        let temp_path1 = temp1.path();
        let temp_path_str1 = temp_path1.to_str().unwrap();

        let temp2 = tempdir().unwrap();
        let temp_path2 = temp2.path();
        let temp_path_str2 = temp_path2.to_str().unwrap();

        let file1 = touch_file_in(temp_path1).unwrap();
        let file1_str = file1.path().file_name().unwrap();
        let file1_str = file1_str.to_str().unwrap();

        let file2 = touch_file_in(temp_path1.join("world")).unwrap();
        let file2_str = file2.path().file_name().unwrap();
        let file2_str = file2_str.to_str().unwrap();

        let _ = cp_dir(temp_path1, temp_path2).unwrap();

        let a = file1_str;
        let b = PathBuf::from("world").join(file2_str);
        let cmds: ArgsList = &[
            (&["add", a], true),
            (&["add", b.to_str().unwrap()], true),
        ];

        let git = &["git", "-C", temp_path_str1];
        let cargo = &["cargo", "run", "--quiet", "--", "-C", temp_path_str2];
        let _ = run_both(cmds, git, cargo).unwrap();

        println!("{}", shell_spawn(&["ls", "-lah", temp_path_str1, temp_path1.join(b.to_str().unwrap()).to_str().unwrap()]).unwrap());

        let origin = shell_spawn(&["git", "-C", temp_path_str1, "ls-files", "--stage"]).unwrap();
        let real = shell_spawn(&["git", "-C", temp_path_str2, "ls-files", "--stage"]).unwrap();

        assert_eq!(origin, real);
    }

    #[test]
    fn test_add_same_file_multi() {
        let temp1 = setup_test_git_dir();
        let temp_path1 = temp1.path();
        let temp_path_str1 = temp_path1.to_str().unwrap();

        let temp2 = tempdir().unwrap();
        let temp_path2 = temp2.path();
        let temp_path_str2 = temp_path2.to_str().unwrap();

        let file1 = touch_file_in(temp_path1).unwrap();
        let file1_str = file1.path().file_name().unwrap();
        let file1_str = file1_str.to_str().unwrap();

        let _ = cp_dir(temp_path1, temp_path2).unwrap();

        let a = file1_str;
        let cmds: ArgsList = &[
            (&["add", a], true),
            (&["add", a], true),
            (&["add", a], true),
            (&["add", a], true),
            (&["add", a], true),
            (&["add", a], true),
            (&["add", a], true),
        ];

        let git = &["git", "-C", temp_path_str1];
        let cargo = &["cargo", "run", "--quiet", "--", "-C", temp_path_str2];
        let _ = run_both(cmds, git, cargo).unwrap();

        println!("{}", shell_spawn(&["ls", "-lah", temp_path_str1]).unwrap());

        let origin = shell_spawn(&["git", "-C", temp_path_str1, "ls-files", "--stage"]).unwrap();
        let real = shell_spawn(&["git", "-C", temp_path_str2, "ls-files", "--stage"]).unwrap();

        assert_eq!(origin, real);
    }
}
