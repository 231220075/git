use std::path::{PathBuf,Path};
use clap::{Parser, Subcommand};
use crate::{
    GitError,
    Result,
};
use crate::utils::{
    fs::{
        read_file_as_bytes,
        get_git_dir,
        write_object,
        calc_relative_path,
    },
    hash::hash_object,
    index::{Index, IndexEntry},
    blob::Blob,
};
use super::SubCommand;
use tempfile::TempDir;

#[derive(Parser, Debug)]
#[command(name = "update-index", about = "update the index file")]
pub struct UpdateIndex {
    #[arg(long, help = "Add file contents to the index")]
    add: bool,

    #[arg(long, help = "Remove file from the index")]
    rm: bool,

    #[arg(long, num_args = 3, help = "Sepcify file mode, hash and name")]
    cacheinfo: Option<Vec<String>>,

    #[arg(help = "Path to the file")]
    names: Vec<String>,

    //#[arg(skip)]
    //gitdir: PathBuf,
}

impl UpdateIndex {
    pub fn from_args(args: impl Iterator<Item = String>)
-> Result<Box<dyn SubCommand>>{
        let update_index = UpdateIndex::try_parse_from(args)?;
        //update_index.gitdir = gitdir;
        Ok(Box::new(update_index))
    }
}


impl SubCommand for UpdateIndex {
    fn run(&self, gitdir: Result<PathBuf>) -> Result<i32> {
        let gitdir = gitdir?;
        let index_path = gitdir.join("index");
        let mut index = Index::new();

        if index_path.exists() {
            index = index.read_from_file(&index_path)?;
        }
        if let Some(cacheinfo) = &self.cacheinfo {
            if cacheinfo.len() != 3 {
                return Err(Box::new(GitError::InvalidCommand("cacheinfo".to_string())));
            }
            let mode = u32::from_str_radix(&cacheinfo[0], 8).map_err(|_| {
                GitError::InvalidCommand("Invalid file mode".to_string())
            })?;
            let hash = cacheinfo[1].clone();
            let name = cacheinfo[2].clone();

            let entry = IndexEntry::new(mode, hash, name);
            index.add_entry(entry);
        }
        else if self.add {
            if self.names.is_empty() {
                return Err(Box::new(GitError::InvalidCommand(
                    "File name is required when using --add".to_string(),
                )));
            }
            for name in &self.names {
                let project_dir = gitdir.parent().unwrap();
                let file_path = project_dir.join(name);

                let bytes = read_file_as_bytes(&project_dir.to_path_buf().join(file_path))?;

                let hash = write_object::<Blob>(gitdir.clone(), bytes)?;
                let mode = 0o100644;
                let path = calc_relative_path(project_dir, name)?;
                let entry = IndexEntry::new(mode, hash, path.to_str().ok_or(GitError::InvaildPathEncoding(name.clone())
                )?.to_string());
                index.add_entry(entry);
            } 
        }
        else if self.rm {
            if self.names.is_empty() {
                return Err(Box::new(GitError::InvalidCommand(
                    "File name is required when using --rm".to_string(),
                )));
            }
            for name in &self.names {
                if !index.remove_entry(name) {
                    return Err(Box::new(GitError::FileNotFound(name.clone())));
                }
            }
        } else {
            return Err(Box::new(GitError::InvalidCommand(
                "Invalid command: either --add, --rm, or --cacheinfo must be specified".to_string(),
            )));
        }

        index.write_to_file(&index_path)?;
        Ok(0)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;
    use crate::utils::test::{
        shell_spawn,
        setup_test_git_dir,
        mktemp_in,
    };


    #[test]
    fn test_update_index_with_cacheinfo() {
        let temp_dir = setup_test_git_dir();
        let index_path = temp_dir.path().join(".git").join("index");

        // 设置当前工作目录
        std::env::set_current_dir(&temp_dir).unwrap();

        // 模拟命令行参数
        let args = vec![
            "update-index".to_string(),
            "--cacheinfo".to_string(),
            "100644".to_string(),
            "d8329fc1cc938780ffdd9f94e0d364e0ea74f579".to_string(),
            "test.txt".to_string(),
        ];

        let update_index = UpdateIndex::try_parse_from(args).unwrap();
        let result = update_index.run(get_git_dir());

        // 验证运行结果
        assert!(result.is_ok());

        // 验证索引文件是否写入
        // println!("Index path: {:?}", index_path); // 调试输出
        // println!("courrent dir: {:?}", std::env::current_dir()); // 调试输出
        // println!("Index path exists: {:?}", index_path.exists()); // 调试输出
        assert!(index_path.exists());
        let index_content = fs::read(&index_path).unwrap();
        assert!(!index_content.is_empty());
    }

    // #[test]
    // fn test_update_index_with_add() {
    //     let temp_dir = setup_test_git_dir();
    //     let index_path = temp_dir.path().join(".git").join("index");
    //     let test_file_path = temp_dir.path().join("test.txt");

    //     // 创建测试文件
    //     fs::write(&test_file_path, b"Hello, world!").unwrap();

    //     // 设置当前工作目录
    //     std::env::set_current_dir(&temp_dir).unwrap();

    //     // 模拟命令行参数
    //     let args = vec![
    //         "update-index".to_string(),
    //         "--add".to_string(),
    //         "test.txt".to_string(),
    //     ];

    //     let update_index = UpdateIndex::try_parse_from(args).unwrap();
    //     println!("{:?}", temp_dir.path().join(".git"));

    //     let result = update_index.run(Ok(temp_dir.path().join(".git")));
    //     println!("result = {:?}", result);
    //     println!("update-index .name = {:?}", update_index.name);
    //     println!("test_file_path = {:?}", test_file_path);
    //     println!("current_path = {:?}", std::env::current_dir());
    //     // 验证运行结果
    //     assert!(result.is_ok());

    //     // 验证索引文件是否写入
    //     assert!(index_path.exists());
    //     let index_content = fs::read(&index_path).unwrap();
    //     assert!(!index_content.is_empty());
    // }

    // #[test]
    // fn test_update_index_missing_file() {
    //     let temp_dir = setup_test_git_dir();

    //     // 设置当前工作目录
    //     std::env::set_current_dir(&temp_dir).unwrap();

    //     // 模拟命令行参数
    //     let args = vec![
    //         "update-index".to_string(),
    //         "--add".to_string(),
    //         "nonexistent.txt".to_string(),
    //     ];

    //     let update_index = UpdateIndex::try_parse_from(args).unwrap();
    //     let result = update_index.run(get_git_dir());

    //     // 验证运行结果
    //     assert!(result.is_err());
    //     if let Err(err) = result {
    //         assert_eq!(
    //             err.to_string(),
    //             "No such file or directory (os error 2)"
    //         );
    //     }
    // }

    #[test]
    fn test_with_simple_add() {
        let temp = setup_test_git_dir();
        let temp_dir = temp.path().to_str().unwrap();
        let gitdir = temp.path().join(".git");

        let file1 = mktemp_in(temp_dir).unwrap();
        let file2 = mktemp_in(temp_dir).unwrap();

        let _ = shell_spawn(&["cargo", "run", "--", "-C", temp_dir, "update-index", "--add", file1.to_str().unwrap()]);
        let out = shell_spawn(&["git", "-C", gitdir.to_str().unwrap(), "ls-files", "--stage"]).unwrap();
        println!("out = {:?}, f1 = {:?}", out, file1);

        let file1_name = std::path::Path::new(file1.to_str().unwrap()).file_name().unwrap().to_str().unwrap();
        assert!(out.contains(file1_name));

        let _ = shell_spawn(&["cargo", "run", "--", "-C", temp_dir, "update-index", "--add", file2.to_str().unwrap()]);
        let out = shell_spawn(&["git", "-C", gitdir.to_str().unwrap(), "ls-files", "--stage"]).unwrap();
        println!("{:?}", out);
        let file2_name = std::path::Path::new(file2.to_str().unwrap()).file_name().unwrap().to_str().unwrap();
        assert!(out.contains(file2_name));
    }

    #[test]
    fn test_with_tree_add() {
        let temp = setup_test_git_dir();
        let temp_dir = temp.path().to_str().unwrap();
        let gitdir = temp.path().join(".git");
        let gitdir = gitdir.to_str().unwrap();

        let file1 = mktemp_in(temp_dir).unwrap();

        let file2 = mktemp_in(temp_dir).unwrap();

        //let _ = shell_spawn(&["cargo", "run", "--", "-C", temp_dir, "update-index", "--add", file1.to_str().unwrap()]).unwrap();
        //let _ = shell_spawn(&["cargo", "run", "--", "-C", temp_dir, "update-index", "--add", file2.to_str().unwrap()]).unwrap();
        let _ = shell_spawn(&["git", "-C", temp_dir, "update-index", "--add", file1.to_str().unwrap()]).unwrap();
        let _ = shell_spawn(&["git", "-C", temp_dir, "update-index", "--add", file2.to_str().unwrap()]).unwrap();
        let file3 = mktemp_in(temp.path().join("dir")).unwrap();
        //let _ = shell_spawn(&["cargo", "run", "--", "-C", temp_dir, "update-index", "--add", file3.to_str().unwrap()]).unwrap();
        let _ = shell_spawn(&["git", "-C", temp_dir, "update-index", "--add", file3.to_str().unwrap()]).unwrap();
        let tree_commit = shell_spawn(&["git", "-C", temp_dir, "write-tree"]).unwrap();
        let tree_commit = tree_commit.trim();
        println!("tree_commit: {}", tree_commit);
        println!("gitdir: {}", gitdir);
        println!("temp_dir: {}", temp_dir);
        let index_dump = shell_spawn(&["git", "-C", temp_dir, "cat-file", "-p", tree_commit]).unwrap();
        println!(".git/index hexdump:\n{}", index_dump);
        let _ = shell_spawn(&["git", "-C", temp_dir, "read-tree", "--prefix=apk", &tree_commit]).unwrap();

        let out = shell_spawn(&["git", "-C", gitdir, "ls-files", "--stage"]).unwrap();
        assert!(out.contains(file1.strip_prefix(temp.path()).unwrap().to_str().unwrap()));
        assert!(out.contains(file2.strip_prefix(temp.path()).unwrap().to_str().unwrap()));
        assert!(out.contains(file3.strip_prefix(temp.path()).unwrap().to_str().unwrap()));
    }

    #[test]
    fn test_with_two_add() {
        let temp = setup_test_git_dir();
        let temp_dir = temp.path().to_str().unwrap();

        let file1 = mktemp_in(temp_dir).unwrap();
        let file1_path = file1.to_str().unwrap();
        let file2 = mktemp_in(temp_dir).unwrap();
        let file2_path = file2.to_str().unwrap();
        let _ = shell_spawn(&["cargo", "run", "--", "-C", temp_dir, "update-index", "--add", file1_path, file2_path]).unwrap();

        let out = shell_spawn(&["git", "-C", temp_dir, "ls-files", "--stage"]).unwrap();
        println!("out = {:?}", out);

        //assert!(out.contains(file1_path.strip_prefix(temp_dir).unwrap()) && out.contains(file2_path.strip_prefix(temp_dir).unwrap()));
        let file1_name = std::path::Path::new(file1_path).file_name().unwrap().to_str().unwrap();
        let file2_name = std::path::Path::new(file2_path).file_name().unwrap().to_str().unwrap();
        println!("file1_name = {:?}, file2_name = {:?}", file1_name, file2_name);
        assert!(out.contains(file1_name) && out.contains(file2_name));
        assert_eq!(out.lines().count(), 2);
    }

    #[test]
    fn test_with_multi_add() {
        let temp = setup_test_git_dir();
        let temp_dir = temp.path().to_str().unwrap();

        let files = (1..100).map(|_| mktemp_in(temp_dir).unwrap())
            .map(|path| path.to_str().unwrap().to_string())
            .collect::<Vec<String>>();

        let mut cmd: Vec<&str> = vec!["cargo", "run", "--", "-C", temp_dir, "update-index", "--add"];
        cmd.extend(files.iter().map(|x|x.as_str()));

        let _ = shell_spawn(&cmd).unwrap();

        let out = shell_spawn(&["git", "-C", temp_dir, "ls-files", "--stage"]).unwrap();
        assert_eq!(out.lines().count(), 99);
    }

    #[test]
    fn test_empty() {
        let temp = setup_test_git_dir();
        let temp_path = temp.path();
        let temp_path_str = temp_path.to_str().unwrap();

        let out = shell_spawn(&["cargo", "run", "--", "-C", temp_path_str, "update-index", "--add", temp_path_str]);
        assert!(out.is_err());
    }


    // #[test]
    // fn test_inner_relative() {
    //     let temp: TempDir = setup_test_git_dir();
    //     let gitdir: PathBuf = temp.path().join(".git");
    //     let gitdir: &str = gitdir.to_str().unwrap();

    //     let file1: PathBuf = mktemp_in(temp.path().join("inner")).unwrap();
    //     let file2: PathBuf = mktemp_in(temp.path().join("inner")).unwrap();
    //     let file1: &str = file1.file_name().unwrap().to_str().unwrap();
    //     let file2: &str = file2.file_name().unwrap().to_str().unwrap();

    //     // enter inner directory
    //     std::env::set_current_dir(temp.path().join("inner")).unwrap();
    //     let git: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    //         .join("target").join("debug").join("git");
    //     let _ = shell_spawn(&[git.to_str().unwrap(), "update-index", "--add", file1]).unwrap();
    //     let _ = shell_spawn(&[git.to_str().unwrap(), "update-index", "--add", file2]).unwrap();

    //     let out: String = shell_spawn(&["git", "-C", gitdir, "ls-files", "--stage"]).unwrap();

    //     println!("{}", out);
    //     assert!(out.contains(PathBuf::from("inner").join(file1).to_str().unwrap()));
    //     assert!(out.contains(PathBuf::from("inner").join(file2).to_str().unwrap()));
    //     //drop(out); drop(git); drop(file2); drop(file1); drop(gitdir); drop(temp);
    // }
}
