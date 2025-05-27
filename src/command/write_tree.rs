use std::path::{PathBuf,Path};
use clap::{Parser, Subcommand};
use crate::{
    GitError,
    Result,
};
use crate::utils::{
    fs::read_file_as_bytes,
    hash::hash_object,
    zlib::compress_object,
    index::{Index, IndexEntry},
    tree::Tree,
};
use super::SubCommand;
use hex;

#[derive(Parser, Debug)]
#[command(name = "write-tree", about = "create a tree object according to the current index")]
pub struct WriteTree {
    //#[arg(skip)]
    //gitdir: PathBuf,
}
impl WriteTree {
    pub fn from_args(args: impl Iterator<Item = String>) -> Result<Box<dyn SubCommand>> {
        let write_tree = WriteTree::try_parse_from(args)?;
        //write_tree.gitdir = gitdir;
        Ok(Box::new(write_tree))
    }

    fn build_tree_content(&self, index: &Index) -> Result<Vec<u8>>{
        let mut tree_content = Vec::new();
        //tree_content.extend_from_slice(b"tree ");
        let mut temp =Vec::new();

        for entry in &index.entries {
            let mode = format!("{:o}", entry.mode);
            temp.extend_from_slice(mode.as_bytes());
            temp.push(b' ');
            temp.extend_from_slice(entry.name.as_bytes());
            temp.push(b'\0');
            let hash_bytes = hex::decode(&entry.hash).map_err(|_| {
                GitError::InvalidCommand(format!("Invalid hash format: {}", entry.hash))
            })?;
            temp.extend_from_slice(&hash_bytes);
        }
        //let len_str = format!("{}\0", temp.len());
        //tree_content.extend_from_slice(len_str.as_bytes());
        tree_content.extend_from_slice(&temp);
        Ok(tree_content)
    }

    fn build_tree_recursive(gitdir: &Path, entries: &[IndexEntry], prefix: &str) -> Result<String>{
        use std::collections::BTreeMap;
        let mut tree_entries: BTreeMap<String, (u32, String, bool)> = BTreeMap::new();
        let mut subdir_map: BTreeMap<String, Vec<IndexEntry>> = BTreeMap::new();

        for entry in entries {
            let rel_name = if prefix.is_empty() {
                entry.name.as_str()
            } else if let Some(stripped) = entry.name.strip_prefix(prefix) {
                stripped.trim_start_matches('/')
            } else {
                continue;
            };
            if let Some((first, rest)) = rel_name.split_once('/') {
                // 子目录
                let _sub_prefix = if prefix.is_empty() {
                    first.to_string()
                } else {
                    format!("{}/{}", prefix, first)
                };
                subdir_map.entry(first.to_string())
                    .or_default()
                    .push(IndexEntry {
                        name: if rest.is_empty() {
                            first.to_string()
                        } else {
                            format!("{}/{}", first, rest)
                        },
                        mode: entry.mode,
                        hash: entry.hash.clone(),
                    });
            } else {
                // 普通文件
                tree_entries.insert(
                    rel_name.to_string(),
                    (entry.mode, entry.hash.clone(), false),
                );
            }
        }
        for (subdir, sub_entries) in subdir_map {
            let sub_prefix = if prefix.is_empty() {
                subdir.clone()
            } else {
                format!("{}/{}", prefix, subdir)
            };
            let sub_tree_hash = Self::build_tree_recursive(gitdir, &sub_entries, &sub_prefix)?;
            tree_entries.insert(
                subdir,
                (0o040000, sub_tree_hash, true),
            );
        }

        let mut tree_content = Vec::new();
        for (name, (mode, hash, is_tree)) in &tree_entries {
            let mode_str = if *is_tree { "40000" } else { &format!("{:o}", mode) };
            tree_content.extend_from_slice(mode_str.as_bytes());
            tree_content.push(b' ');
            tree_content.extend_from_slice(name.as_bytes());
            tree_content.push(0);
            let hash_bytes = hex::decode(hash).map_err(|_| {
                GitError::InvalidCommand(format!("Invalid hash format: {}", hash))
            })?;
            tree_content.extend_from_slice(&hash_bytes);
        }

        let tree_hash = hash_object::<Tree>(tree_content.clone())?;
        let mut objpath = gitdir.join("objects");
        objpath.push(&tree_hash[0..2]);
        objpath.push(&tree_hash[2..]);
        //println!("objpath: {:?}", objpath);
        std::fs::create_dir_all(objpath.parent().unwrap())?;
        //println!("objpath: {:?}", objpath);
        let compressed = compress_object::<Tree>(tree_content)?;
        //println!("compressed: {:?}", compressed);
        std::fs::write(objpath, compressed)?;
        //println!("tree_hash: {}", tree_hash);
        Ok(tree_hash)

    }

    pub fn lazy_fucker(gitdir: PathBuf) -> Result<String> {
        let index_path = gitdir.join("index");
        let index = Index::new();
        let index = index.read_from_file(&index_path)?;
        Self::build_tree_recursive(&gitdir, &index.entries, "")
    }
}

impl SubCommand for WriteTree {
    // fn run(&self, gitdir: Result<PathBuf>) -> Result<i32> {
    //     let gitdir = gitdir?;
    //     let index_path =gitdir.clone().join("index");
    //     let index = Index::new();
    //     let index = index.read_from_file(&index_path).map_err(|_| {
    //         GitError::InvalidCommand(index_path.to_str().unwrap().to_string())
    //     })?;
        
    //     let tree_content = self.build_tree_content(&index)?;
    //     let tree_hash = hash_object::<Tree>(tree_content.clone())?;
    //     //let mut objpath = self.gitdir.join("objects");
    //     let mut objpath = gitdir.clone().join("objects");
    //     objpath.push(&tree_hash[0..2]);
    //     objpath.push(&tree_hash[2..]);
    //     std::fs::create_dir_all(objpath.parent().unwrap())?;
        
    //     let compressed = compress_object::<Tree>(tree_content)?;
        
    //     std::fs::write(objpath, compressed)?;
    //     println!("{}", tree_hash);
    //     Ok(0)
    // }

   fn run(&self, gitdir: Result<PathBuf>) -> Result<i32> {
        let gitdir = gitdir?;
        let index_path = gitdir.clone().join("index");
        let index = Index::new();
        let index = index.read_from_file(&index_path)?;
        let tree_hash = Self::build_tree_recursive(&gitdir, &index.entries, "")?;
        println!("{}", tree_hash);
        Ok(0)
    }


}

#[cfg(test)]
mod test {
    use crate::utils::test::{
        shell_spawn,
        setup_test_git_dir,
        mktemp_in,
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
            (&["update-index", "--add", file1_str, file2_str], false),
            (&["write-tree"], true),
        ];
        let git = &["git", "-C", temp_path_str1];
        let cargo = &["cargo", "run", "--quiet", "--", "-C", temp_path_str2];
        let (gitout, _) = run_both(cmds, git, cargo).unwrap();

        let hash = gitout.iter().filter(|x|x.len() == 41).take(1).next().unwrap().strip_suffix("\n").unwrap();

        let real = shell_spawn(&["git", "-C", temp_path_str1, "cat-file", "-p", &hash]).unwrap();
        let origin = shell_spawn(&["git", "-C", temp_path_str2, "cat-file", "-p", &hash]).unwrap();
        assert_eq!(origin, real);
    }
}
