use std::fs::write;
use std::iter::Peekable;
use std::io;
use std::cmp::min;
use std::path::{
    Path,
    PathBuf
};
use clap::{Parser, Subcommand};
use itertools::Itertools;
use similar::{ChangeTag, TextDiff};
use diffy::{
    MergeOptions,
    ConflictStyle,
};
use crate::utils::{
    zlib::{
        decompress_file,
        compress_object
    },
    fs::{
        obj_to_pathbuf,
        read_file_as_bytes,
        write_object,
        read_obj,
        read_object,
    },
    hash::hash_object,
    objtype::{
        ObjType,
        Obj,
    },
    refs::{
        head_to_hash,
        read_ref_commit,
        write_ref_commit,
        write_branch_commit,
        write_head_ref,
        read_head_ref,
        read_branch_commit,
    },
    index::{
        Index,
        IndexEntry,
    },
    blob::Blob,
    tree::{
        Tree,
        FileMode,
        TreeEntry,
    },
    commit::Commit,
    test::shell_spawn,
};

use crate::command::{
    commit,
    update_ref,
    checkout::Checkout,
};
use crate::{
    GitError,
    Result,
};
use super::SubCommand;

type Diffence = (Option<Vec<TreeEntry>>, Option<Vec<(TreeEntry, TreeEntry)>>);


#[derive(Parser, Debug)]
#[command(name = "merge", about = "Join two or more development histories together")]
pub struct Merge {

    #[arg(required = true, help = "branch name you want to merge into HEAD")]
    branch: String
}

impl Merge {
    pub fn from_args(args: impl Iterator<Item = String>) -> Result<Box<dyn SubCommand>> {
        Ok(Box::new(Merge::try_parse_from(args)?))
    }

    fn get_all_ancestor<P>(gitdir: P, hash: Option<String>, mut sofar: Vec<String>) -> Result<Vec<String>>
    where
        P: AsRef<Path>
    {
        if hash.is_none() {
            // println!("return {:?}", sofar);
            Ok(sofar)
        }
        else {
            let hash = hash.unwrap();
            if let Obj::C(Commit {parent_hash,..}) = read_obj(gitdir.as_ref().to_path_buf(), &hash)? {
                sofar.insert(0, hash);
                Self::get_all_ancestor(gitdir, if !parent_hash.is_empty() {Some(parent_hash[0].clone())} else {None}, sofar)
            }
            else {
                Err(GitError::broken_commit_history(hash))
            }
        }
    }

    fn first_same_commit(gitdir: impl AsRef<Path>, hash1: String, hash2: String) -> Result<String> {
        let ancestor1 = Self::get_all_ancestor(&gitdir, Some(hash1.clone()), Vec::new())?;
        let ancestor2 = Self::get_all_ancestor(&gitdir, Some(hash2.clone()), Vec::new())?;
        let index = ancestor1.iter()
            .zip(ancestor2.iter()) // 将两个数组的元素一一配对
            .take_while(|(a, b)| a == b) // 取出相等的元素，直到遇到不相等的为止
            .count();

        if index >= 1 {
            Ok(ancestor1[index - 1].clone())
        }
        else {
            Err(GitError::no_same_ancestor(format!("can not find same ancestor for {} and {}", hash1, hash2)))
        }
    }

    fn fast_forward(gitdir: impl AsRef<Path>, branch_name: &str, original_branch: &str) -> Result<()> {
        let hash = read_branch_commit(gitdir.as_ref(), branch_name)?;

        let checkout = Checkout::from_internal(Some(branch_name.to_string()), vec![]);
        checkout.run(Ok(gitdir.as_ref().to_path_buf()))?;

        write_ref_commit(gitdir.as_ref(), original_branch, &hash)?;
        write_head_ref(gitdir.as_ref(), original_branch)?;
        println!("{hash}");

        Ok(())
    }

    fn diff_array<T>(mut a: Peekable<T>, mut b: Peekable<T>) -> Diffence
    where
        T: Iterator<Item = TreeEntry>,
    {
        let head_a = a.peek();
        let head_b = b.peek();
        match (head_a, head_b) {
            (None, None) => (None, None),
            (Some(_), None) => (Some(a.collect::<Vec<_>>()), None),
            (None, Some(_)) => (Some(b.collect::<Vec<_>>()), None),

            (Some(head_a), Some(head_b)) if head_a == head_b => {
                let (head_a, head_b) = (a.next().unwrap(), b.next().unwrap());
                match Self::diff_array(a, b) {
                    (diffence, Some(mut same)) => {
                        same.push((head_a, head_b));
                        (diffence, Some(same))
                    },
                    (diffence, None) => (diffence, Some(vec![(head_a, head_b)])),
                }
            },
            (Some(head_a), Some(head_b)) if head_a < head_b => {
                let head_a = a.next().unwrap();
                match Self::diff_array(a, b) {
                    (Some(mut diffence), same) => {
                        diffence.push(head_a);
                        (Some(diffence), same)
                    },
                    (None, same) => (Some(vec![head_a]), same),
                }
            },
            (Some(_), Some(_)) => {
                let head_b = b.next().unwrap();
                match Self::diff_array(a, b) {
                    (Some(mut diffence), same) => {
                        diffence.push(head_b);
                        (Some(diffence), same)
                    },
                    (None, same) => (Some(vec![head_b]), same)
                }
            },
        }
    }

    fn handle_dirrence_file(index: &mut Index, diffence: Option<Vec<TreeEntry>>) {
        if let Some(diffence) = diffence {
            diffence.into_iter()
                .for_each(|TreeEntry{mode, hash, path}| {
                    // println!("save {} to stage", path.display());
                    index.add_entry(IndexEntry::new(mode as u32, hash, path.display().to_string()));
                })
        };
    }

    fn diff_text(original: &str, modified: &str) -> Vec<Vec<usize>> {
        let origin = original.split("\n")
            .collect::<Vec<_>>();
        let modify = modified.split("\n")
            .collect::<Vec<_>>();

        let (origin_len, modify_len) = (origin.len(), modify.len());
        let minimal = min(origin_len, modify_len);
        let (origin, ori_rest) = origin.split_at(minimal);
        let (modify, mod_rest) = modify.split_at(minimal);
        // println!("ori_rest = {:?}, mod_rest = {:?}", ori_rest, mod_rest);

        let original = origin.iter().cloned().map(String::from);
        let modified = modify.iter().cloned().map(String::from);

        let mut ranges: Vec<Vec<usize>> = vec![];
        let acc = original.zip(modified)
            .map(|(a, b)| (a == b) as u32)
            .enumerate()
            .filter(|(_, x)| *x == 0)
            .map(|(i, _)| i + 1)
            .fold(Vec::new(), |mut acc, ele| {
                if (acc.is_empty()) || (acc[acc.len() - 1] + 1 == ele) {
                    acc.push(ele);
                    acc
                }
                else {
                    ranges.push(acc);
                    vec![ele]
                }
            });

        if !acc.is_empty() {
            ranges.push(acc);
        }
        if ori_rest.len() > mod_rest.len() {
            ranges.push(vec![minimal + 1, minimal + mod_rest.len() - 1]);
        }
        else if ori_rest.len() < mod_rest.len() {
            ranges.push(vec![minimal + 1, minimal + ori_rest.len() - 1]);
        }
        else if ori_rest.len() == 1 {
            ranges.push(vec![minimal + 1]);
        }
        else if !ori_rest.is_empty() {
            ranges.push(vec![minimal + 1, minimal + ori_rest.len() - 1]);
        }
        ranges
    }

    fn save_conflict_object(index: &mut Index, gitdir: PathBuf, a: &TreeEntry, b: &TreeEntry, a_blob: &str, b_blob: &str) -> Result<()> {
        index.add_entry(IndexEntry::new(a.mode as u32, a.hash.clone(), a.path.display().to_string()));
        index.add_entry(IndexEntry::new(b.mode as u32, b.hash.clone(), b.path.display().to_string()));
        // println!("add {}", a.path.display());
        // println!("add {}", b.path.display());
        let mut mo = MergeOptions::new();
        mo.set_conflict_style(ConflictStyle::Merge);
        if let Err(diff) = mo.merge("", a_blob, b_blob) {
            let hash = write_object::<Blob>(gitdir.clone(), diff.into_bytes())?;
            // println!("add {}", hash);
            index.add_entry({
                IndexEntry {
                    mode: a.mode as u32,
                    hash,
                    name: a.path.display().to_string()
                }
            })
        }
        Ok(())
    }

    #[allow(clippy::manual_try_fold)]
    fn handle_same_file(index: &mut Index, gitdir: PathBuf, same: Vec<(TreeEntry, TreeEntry)>) -> Result<()> {
        let (equal, not): (Vec<_>, Vec<_>) = same.into_iter().partition(|(a, b)|a.hash == b.hash);
        equal.iter()
            .for_each(|(a, _)| {
                // println!("add {}", a.path.display());
                index.add_entry(IndexEntry::new(a.mode as u32, a.hash.clone(), a.path.display().to_string()));
            });

        let (_, err): (Vec<_>, Vec<_>) = not.into_iter()
            .map(|(a, b)| {
                let a_blob = String::from_utf8(read_object::<Blob>(gitdir.clone(), &a.hash)?.into())?;
                let b_blob = String::from_utf8(read_object::<Blob>(gitdir.clone(), &b.hash)?.into())?;
                Self::save_conflict_object(index, gitdir.clone(), &a, &b, &a_blob, &b_blob)?;

                let output = Self::diff_text(&a_blob, &b_blob)
                    .into_iter()
                    .map(|v| {
                        // println!("v.len() = {}", v.len());
                        if v.len() == 1 {
                            format!("Merge conflict in {}: {}", a.path.display(), v[0])
                        }
                        else {
                            format!("Merge conflict in {}: [{}, {}]", a.path.display(), v[0], v[v.len() - 1])
                        }
                    })
                    .collect::<Vec<String>>()
                    .join("\n");
                Err(GitError::merge_conflict(output))
            })
            .partition(|x: &Result<_>|x.is_ok());

        if !err.is_empty() {
            err.into_iter()
                .fold(Err(GitError::merge_conflict("".to_string())), |acc, ele: Result<()>| {
                    match (acc, ele) {
                        (Err(a), Err(b)) => {
                            Err(GitError::merge_conflict( {
                                let out = a.to_string() + "\n" + &b.to_string();
                                if out.starts_with("\n") {
                                    out.strip_prefix("\n").unwrap().to_string()
                                }
                                else {
                                    out
                                }
                            }))
                        },
                        _ => Ok(())
                    }
                })
        }
        else {
            Ok(())
        }
    }

    fn merge_tree(gitdir: PathBuf, hash_a: String, hash_b: String) -> Result<Index> {
        let tree_a = read_object::<Tree>(gitdir.clone(), &hash_a)?;
        let tree_b = read_object::<Tree>(gitdir.clone(), &hash_b)?;
        // println!("tree_a = {}", tree_a);

        let paths_a = tree_a.into_iter_flatten(gitdir.clone())?.into_iter().sorted();
        let paths_b = tree_b.into_iter_flatten(gitdir.clone())?.into_iter().sorted();
        let (diffence, same) = Self::diff_array(paths_a.peekable(), paths_b.peekable());

        // overwirte the index file
        let mut index = Index::new();
        Self::handle_dirrence_file(&mut index, diffence);
        if let Some(same) = same {
            let result = Self::handle_same_file(&mut index, gitdir.clone(), same);
            if result.is_err() {
                // println!("before writing to index file, index.len = {}", index.entries.len());
                index.write_to_file(&gitdir.join("index"))?;
            }
            result?;
        }
        // println!("before writing to index file, index.len = {}", index.entries.len());
        index.write_to_file(&gitdir.join("index"))?;
        Ok(index)
    }

}


impl SubCommand for Merge {
    fn run(&self, gitdir: Result<PathBuf>) -> Result<i32> {
        let gitdir = gitdir?;
        let hash1 = head_to_hash(&gitdir)?;
        let hash2 = read_ref_commit(&gitdir, &format!("refs/heads/{}", self.branch))?;
        let base_hash = Self::first_same_commit(&gitdir, hash1.clone(), hash2.clone())?;

        if base_hash == hash2 {
            //println!("it's already latest");
        }
        else if base_hash == hash1 {
            //println!("fast forward");
            let original_branch = read_head_ref(&gitdir)?;
            Self::fast_forward(&gitdir, &self.branch, &original_branch)?;
        }
        else {
            // | --- | base  | a     | b     |
            // | --- | ---   | ---   | ---   |
            // | 1   | True  | True  | True  |
            // | 5   | False | True  | True  |

            // | 2   | True  | True  | False |
            // | 3   | True  | False | True  |
            // | 4   | True  | False | False |
            // | 6   | False | True  | False |
            // | 7   | False | False | True  |

            let commit_a = read_object::<Commit>(gitdir.clone(), &hash1)?;
            let commit_b = read_object::<Commit>(gitdir.clone(), &hash2)?;
            let index = Self::merge_tree(gitdir.clone(), commit_a.tree_hash, commit_b.tree_hash)?;

            // make a new commit
            let tree = Tree({
                index.entries
                .into_iter()
                .map(|IndexEntry {mode, hash, name}| TreeEntry {
                    mode: mode.try_into().unwrap(),
                    hash,
                    path: PathBuf::from(name),
                })
                .collect::<Vec<TreeEntry>>()
            });
            let tree_hash = write_object::<Tree>(gitdir.clone(), tree.into())?;

            let commit = Commit {
                tree_hash,
                parent_hash: vec![hash1, hash2],
                author: "Default Author <139881912@163.com> 1748165415 +0800".into(),
                committer: "commiter Author <139881912@163.com> 1748165415 +0800".into(),
                message: format!("merge {} into this\n", self.branch)
            };
            let merge_hash = write_object::<Commit>(gitdir.clone(), commit.into())?;

            let update_ref = update_ref::UpdateRef {
                ref_path: read_head_ref(&gitdir)?,
                commit_hash: merge_hash.clone(),
            };
            update_ref.run(Ok(gitdir.clone()))?;
            println!("{}", merge_hash);

            // Checkout::restore_workspace(&gitdir, &merge_hash)?;
            let head = read_head_ref(&gitdir)?;
            let checkout = Checkout::from_internal(Some(head), vec![]);
            checkout.run(Ok(gitdir))?;
        }
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
    fn test_fast_forward() {
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

        let a = file1_str;
        let b = PathBuf::from("world").join(file2_str);
        let setup_cmds: ArgsList = &[
            (&["add", a], false),
            (&["commit", "-m", "branch master"], false),
            (&["checkout", "-b", "new_branch"], false),
            (&["add", b.to_str().unwrap()], false),
            (&["commit", "-m", "branch new"], false),
            (&["checkout", "master"], false),
        ];

        let git = &["git", "-C", &temp_path_str1];
        let mut oper = cmd_seq(setup_cmds);
        let _ = oper(git).unwrap();
        let _ = cp_dir(temp_path1, temp_path2).unwrap();

        let test_cmds: ArgsList = &[
            (&["merge", "new_branch"], true),
        ];
        let cargo = &["cargo", "run", "--quiet", "--", "-C", temp_path_str2];
        let _ = run_both(test_cmds, git, cargo).unwrap();

        println!("{}", shell_spawn(&["ls", "-lah", temp_path_str1, temp_path1.join(b.to_str().unwrap()).to_str().unwrap()]).unwrap());

        let origin = shell_spawn(&["git", "-C", temp_path_str1, "ls-files", "--stage"]).unwrap();
        let real = shell_spawn(&["git", "-C", temp_path_str2, "ls-files", "--stage"]).unwrap();

        assert_eq!(origin, real);
    }

    #[test]
    fn test_simple_merge() {
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

        let file3 = touch_file_in(temp_path1.join("world")).unwrap();
        let file3_str = file3.path().file_name().unwrap();
        let file3_str = file3_str.to_str().unwrap();

        let a = file1_str;
        let b = PathBuf::from("world").join(file2_str);
        let c = PathBuf::from("world").join(file3_str);
        let setup_cmds: ArgsList = &[
            (&["add", a], false),
            (&["commit", "-m", "branch master"], false),
            (&["checkout", "-b", "A"], false),
            (&["add", b.to_str().unwrap()], false),
            (&["commit", "-m", "A"], false),
            (&["checkout", "master"], false),
            (&["add", c.to_str().unwrap()], false),
            (&["commit", "-m", "B"], false),
        ];

        let git = &["git", "-C", &temp_path_str1];
        let mut oper = cmd_seq(setup_cmds);
        let _ = oper(git).unwrap();
        let _ = cp_dir(temp_path1, temp_path2).unwrap();

        let test_cmds: ArgsList = &[
            (&["merge", "A"], true),
        ];
        let cargo = &["cargo", "run", "--quiet", "--", "-C", temp_path_str2];
        let _ = run_both(test_cmds, git, cargo).unwrap();

        println!("{}", shell_spawn(&["ls", "-lah", temp_path_str1, temp_path1.join(b.to_str().unwrap()).to_str().unwrap()]).unwrap());

        let origin = shell_spawn(&["git", "-C", temp_path_str1, "ls-files", "--stage"]).unwrap();
        let real = shell_spawn(&["git", "-C", temp_path_str2, "ls-files", "--stage"]).unwrap();
        assert_eq!(
            real.split("\n")
                .into_iter()
                .sorted()
                .collect::<String>(),
            origin.split("\n")
                .into_iter()
                .sorted()
                .collect::<String>()
        );
    }

    #[test]
    fn test_conflict() {
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

        let file3 = touch_file_in(temp_path1.join("world")).unwrap();
        let file3_str = file3.path().file_name().unwrap();
        let file3_str = file3_str.to_str().unwrap();

        let a = file1_str;
        let b = PathBuf::from("world").join(file2_str);
        let c = PathBuf::from("world").join(file3_str);
        let setup_cmds: ArgsList = &[
            (&["add", a], false),
            (&["commit", "-m", "branch master"], false),
            (&["checkout", "-b", "A"], false),
            (&["add", b.to_str().unwrap()], false),
            (&["commit", "-m", "A"], false),
            (&["checkout", "master"], false),
            (&["add", c.to_str().unwrap()], false),
            (&["commit", "-m", "B"], false),
        ];

        let git = &["git", "-C", &temp_path_str1];
        let mut oper = cmd_seq(setup_cmds);
        let _ = oper(git).unwrap();
        let _ = cp_dir(temp_path1, temp_path2).unwrap();

        let test_cmds: ArgsList = &[
            (&["merge", "A"], true),
        ];
        let cargo = &["cargo", "run", "--quiet", "--", "-C", temp_path_str2];
        let _ = run_both(test_cmds, git, cargo).unwrap();

        // println!("{}", shell_spawn(&["ls", "-lah", temp_path_str1, temp_path1.join(b.to_str().unwrap()).to_str().unwrap()]).unwrap());

        let origin = shell_spawn(&["git", "-C", temp_path_str1, "ls-files", "--stage"]).unwrap();
        let real = shell_spawn(&["git", "-C", temp_path_str2, "ls-files", "--stage"]).unwrap();

        println!("origin = {origin}");
        println!("real = {real}");
        assert_eq!(
            real.split("\n")
                .into_iter()
                .sorted()
                .collect::<String>(),
            origin.split("\n")
                .into_iter()
                .sorted()
                .collect::<String>()
        );

        let origin = shell_spawn(&["find",  temp_path_str1, "-maxdepth", "1", "-type", "f", "-printf", "\"%P\n\""]).unwrap();
        let real = shell_spawn(&["find",  temp_path_str2, "-maxdepth", "1", "-type", "f", "-printf", "\"%P\n\""]).unwrap();
        println!("{origin}");
        println!("{real}");
        assert_eq!(
            real.split("\n")
                .into_iter()
                .sorted()
                .collect::<String>(),
            origin.split("\n")
                .into_iter()
                .sorted()
                .collect::<String>()
        );
        // assert!(false);
    }

    #[test]
    fn test_ppt_merge() -> Result<()> {
        let temp_dir = tempdir()?;
        let curr_dir = std::env::current_dir().unwrap();

        shell_spawn(&["mkdir", temp_dir.path().join("tests").to_str().unwrap()])?;
        shell_spawn(&["cp", curr_dir.join("target/debug/git").to_str().unwrap(), temp_dir.path().join("tests").join("rust-git").to_str().unwrap()])?;
        shell_spawn(&["chmod", "a+x", temp_dir.path().join("tests").join("rust-git").to_str().unwrap()])?;

        std::env::set_current_dir(&temp_dir)?;
        println!("output = {}", shell_spawn(&[curr_dir.join("tests/test_merge").to_str().unwrap()])?);
        Ok(())
    }
}
