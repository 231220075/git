use std::path::{
    PathBuf,
    Path
};

use clap::{Parser, Subcommand};
use crate::{
    command::ReadTree,
    GitError,
    Result,
    utils::refs::{
        read_head_ref,
        write_head_ref,
        read_ref_commit,
        write_ref_commit,
        write_head_commit,
        read_head_commit,
    },
};
use super::SubCommand;
use std::{
    fs,
    fs::File,
    io::Write,
    os::unix::fs::PermissionsExt,
};

use crate::utils::{
    tree::{
        Tree,
        FileMode,
        TreeEntry,
    },
    zlib::decompress_file_bytes,
    blob::Blob,
    index::Index,
    hash::hash_object,
    index::IndexEntry,
    commit::Commit,
    fs::{
        write_object,
        read_object,
        calc_relative_path,
    }
};

#[derive(Parser, Debug)]
#[command(name = "checkout", about = "切换分支")]
pub struct Checkout {
    #[arg(short = 'b', long = "branch", help = "创建新分支")]
    create_new_branch: bool,

    #[arg(help = "分支名", required = false)]
    branch_name_or_commit_hash: Option<String>,

    #[arg(help = "文件/目录名", required = false)]
    paths: Vec<String>,
}

impl Checkout {
    pub fn from_internal(branch_name: Option<String>, paths: Vec<String>) -> Self {
        Checkout {
            create_new_branch: false,
            branch_name_or_commit_hash: branch_name,
            paths,
        }
    }

    pub fn read_tree(gitdir: &Path, hash: String) -> Result<Tree> {
        read_object::<Tree>(gitdir.to_path_buf(), &hash)
            .map_err(|_| GitError::invalid_command(format!("failed to parse tree data for {}", hash)))
    }

    pub fn read_blob(gitdir: &Path, hash: &str) -> Result<Blob> {
        read_object::<Blob>(gitdir.to_path_buf(), hash)
            .map_err(|_| GitError::invalid_command(format!("failed to parse blob data for {}", hash)))
    }

    pub fn read_commit(gitdir: &Path, hash: &str) -> Result<(Commit, Tree)> {
        let commit = read_object::<Commit>(gitdir.to_path_buf(), hash)
            .map_err(|_| GitError::invalid_command(format!("failed to parse commit data for {}", hash)))?;

        let tree_hash = commit.tree_hash.clone();
        Ok((commit, Self::read_tree(gitdir, tree_hash)?))
    }

    pub fn from_args(args: impl Iterator<Item = String>) -> Result<Box<dyn SubCommand>> {
        Ok(Box::new(Checkout::try_parse_from(args)?))
    }

    pub fn restore_workspace(gitdir: &PathBuf, commit_hash: &str) -> Result<()> {
        let (_, tree) = Self::read_commit(gitdir, commit_hash)?;

                // 获取当前 index
        let index_path = gitdir.join("index");
        let index = Index::new().read_from_file(&index_path).map_err(|_| {
            GitError::failed_to_read_file(&index_path.to_string_lossy())
        })?;

        // 删除 index 中记录的所有文件
        for entry in &index.entries {
            let file_path = gitdir.parent().unwrap().join(&entry.name);
            if file_path.is_file() {
                fs::remove_file(&file_path).map_err(|_| {
                    GitError::failed_to_write_file(&file_path.to_string_lossy())
                })?;
            } else if file_path.is_dir() {
                fs::remove_dir_all(&file_path).map_err(|_| {
                    GitError::failed_to_write_file(&file_path.to_string_lossy())
                })?;
            }
        }
        Checkout::restore_tree(gitdir, &PathBuf::from("."), &tree)?;
        Ok(())
    }

    fn extract_tree_hash(data: &[u8]) -> Option<String> {
        let mut content = String::from_utf8_lossy(data).to_string();
        content = content.replace("tree ", "\ntree ");
        // //println!("content: {}", content);
        for line in content.lines() {
            if let Some(hash) = line.strip_prefix("tree ") {
                return Some(hash.to_string()); // 提取 tree_hash
            }
        }

        None 
    }

    fn restore_tree(gitdir: &PathBuf, base_path:&Path, tree: &Tree) -> Result<()> {
        for entry in &tree.0 {
            //println!("entry: {:?}", entry);
            let file_path = base_path.join(&entry.path);

            match entry.mode {
                FileMode::Blob =>{
                    let blob = Self::read_blob(gitdir, &entry.hash)?;
                    let content: Vec<u8> = blob.into();
                    //println!("content: {:?}", content);
                    fs::write(&file_path, content)
                        .map_err(|_| GitError::failed_to_write_file(&file_path.to_string_lossy()))?;
                },
                FileMode::Exec =>{
                    let blob = Self::read_blob(gitdir, &entry.hash)?;
                    let content: Vec<u8> = blob.into();
                    let mut file = File::create(&file_path)?;
                    file.write_all(&content)?;

                    let mut permissions = file.metadata()?.permissions();
                    permissions.set_mode(FileMode::Exec as u32); // 设置权限为 rwxr-xr-x (八进制表示)
                    file.set_permissions(permissions)?;

                },
                FileMode::Tree => {
                    fs::create_dir_all(&file_path)
                        .map_err(|_| GitError::failed_to_write_file(&file_path.to_string_lossy()))?;
                    let sub_tree = Self::read_tree(gitdir, entry.hash.clone())?;
                    Checkout::restore_tree(gitdir, &file_path, &sub_tree)?;
                },
                _ => {
                    return Err(GitError::invalid_command(format!("unsupported file mode: {:?}", entry.mode)));
                },
            }
        }
        Ok(())
    }

    fn get_staged_file(gitdir: &Path, path: &Path) -> Result<Option<Vec<u8>>> {
        let index_path = gitdir.join("index");

        let index = Index::new().read_from_file(&index_path).map_err(|_| {
            GitError::failed_to_read_file(&index_path.to_string_lossy())
        })?;

        
        if let Some(entry) = index.entries.iter().find(|e| e.name == path.to_string_lossy()) {
            let blob = Self::read_blob(gitdir, &entry.hash)?;
            let content = Vec::<u8>::from(blob);
            return Ok(Some(content));
        }
        Ok(None)
    }

    fn is_workspace_modified(gitdir: &PathBuf) -> Result<bool> {
        let index_path = gitdir.join("index");
        let index = Index::new().read_from_file(&index_path).map_err(|_| {
            GitError::failed_to_read_file(&index_path.to_string_lossy())
        })?;

        // 遍历 index 中的所有条目
        for entry in &index.entries {
            let file_path = PathBuf::from(&entry.name);

            // 检查工作区中是否存在对应的文件
            if !file_path.exists() {
                //println!("File deleted: {:?}", file_path);
                return Ok(true); // 文件被删除
            }

            // 如果是文件（blob），计算文件哈希并比较
            if entry.mode == 0o100644 {
                let file_content = fs::read(&file_path).map_err(|_| {
                    GitError::failed_to_read_file(&file_path.to_string_lossy())
                })?;
                let file_hash = hash_object::<Blob>(file_content)?;
                if file_hash != entry.hash {
                    //println!("File modified: {:?}", file_path);
                    return Ok(true); // 文件内容不同
                }
            }

            // 如果是目录（tree），递归检查子条目
            if entry.mode == 0o40000 {
                let tree = Self::read_tree(gitdir, entry.hash.clone())?;
                if Self::is_workspace_modified_for_tree(gitdir, &file_path, &tree)? {
                    return Ok(true);
                }
            }
        }

        Ok(false) // 工作区和 index 一致
    }

    fn is_workspace_modified_for_tree(gitdir: &PathBuf, base_path: &Path, tree: &Tree) -> Result<bool> {
        for entry in &tree.0 {
            let file_path = base_path.join(&entry.path);

            // 检查工作区中是否存在对应的文件
            if !file_path.exists() {
                //println!("File deleted: {:?}", file_path);
                return Ok(true); // 文件被删除
            }

            // 如果是文件（blob），计算文件哈希并比较
            if entry.mode == FileMode::Blob {
                let file_content = fs::read(&file_path).map_err(|_| {
                    GitError::failed_to_read_file(&file_path.to_string_lossy())
                })?;
                let file_hash = hash_object::<Blob>(file_content)?;
                if file_hash != entry.hash {
                    //println!("File modified: {:?}", file_path);
                    return Ok(true); // 文件内容不同
                }
            }

            // 如果是目录（tree），递归检查子条目
            if entry.mode == FileMode::Tree {
                let sub_tree = Self::read_tree(gitdir, entry.hash.clone())?;
                if Self::is_workspace_modified_for_tree(gitdir, &file_path, &sub_tree)? {
                    return Ok(true);
                }
            }
        }

        Ok(false) // 工作区和 tree 一致
    }
    fn is_index_modified(gitdir: &Path, tree: &Tree) -> Result<bool> {
        // 读取 index 文件
        let index_path = gitdir.join("index");
        let index = Index::new().read_from_file(&index_path).map_err(|_| {
            GitError::failed_to_read_file(&index_path.to_string_lossy())
        })?;

        // 创建一个集合存储 tree 中的所有文件路径
        let tree_paths: std::collections::HashSet<_> = tree.0.iter().map(|entry| &entry.path).collect();

        // 遍历 tree 中的文件
        for entry in &tree.0 {
            if let Some(index_entry) = index.entries.iter().find(|e| e.name == entry.path.to_string_lossy()) {
                // 比较 tree 文件的哈希值与 index 中的哈希值
                if entry.hash != index_entry.hash {
                    return Ok(true); // 文件内容不同
                }
            } else {
                // 如果 tree 中的文件在 index 中不存在
                return Ok(true); // 文件缺失
            }
        }

        // 检查 index 中是否有多余的条目
        for index_entry in &index.entries {
            if !tree_paths.contains(&PathBuf::from(&index_entry.name)) {
                return Ok(true); // 多余的文件
            }
        }

        Ok(false) // index 和 tree 一致
    }

    fn merge_tree_into_index_wrapper(gitdir: &Path, tree: &Tree, prefix: &Path) -> Result<()> {
        let index_path = gitdir.join("index");
        let mut index = Index::new().read_from_file(&index_path).map_err(|_| {
            GitError::failed_to_read_file(&index_path.to_string_lossy())
        })?;

        Checkout::merge_tree_into_index(gitdir, tree, prefix, &mut index)?;

        index.write_to_file(&index_path).map_err(|_| {
            GitError::failed_to_write_file(&index_path.to_string_lossy())
        })?;

        Ok(())
    }

    fn merge_tree_into_index(gitdir: &Path, tree: &Tree, prefix: &Path, index: &mut Index) -> Result<()> {

        for entry in &tree.0 {
            let entry_path = prefix.join(&entry.path); // 添加前缀到当前条目路径

            if entry.mode == FileMode::Tree {
                // 如果是子目录（tree），递归处理
                let sub_tree = Checkout::read_tree(gitdir, entry.hash.clone())?;
                Self::merge_tree_into_index(gitdir, &sub_tree, &entry_path, index)?; // 递归调用时传递当前路径作为前缀
            } else if entry.mode == FileMode::Blob {
                // 如果是文件（blob），检查是否已存在于 index 中
                if index.entries.iter().any(|e| e.name == entry_path.to_string_lossy()) {
                    // 如果 index 中已存在该条目，则跳过
                    continue;
                }

                // 如果 index 中不存在该条目，添加新的条目
                index.entries.push(IndexEntry {
                    name: entry_path.to_string_lossy().to_string(),
                    mode: entry.mode as u32,
                    hash: entry.hash.clone(),
                });
            } else {
                // 如果是其他类型，返回错误
                return Err(GitError::invalid_command(format!(
                    "Unsupported file mode: {:?}",
                    entry.mode
                )));
            }
        }

        Ok(())
    }

    fn merge_index_into_workspace(gitdir: &PathBuf) -> Result<()> {
        let index_path = gitdir.join("index");
        let index = Index::new().read_from_file(&index_path).map_err(|_| {
            GitError::failed_to_read_file(&index_path.to_string_lossy())
        })?;

        for entry in &index.entries {
            let file_path = PathBuf::from(&entry.name);

            match entry.mode {
                0o100644 => {
                    // 如果是文件（blob），处理文件内容
                    if let Some(parent) = file_path.parent() {
                    // 确保父目录存在
                        fs::create_dir_all(parent).map_err(|_| {
                            GitError::failed_to_write_file(&parent.to_string_lossy())
                        })?;
                    }
                    if file_path.exists() {
                        let file_content = fs::read(&file_path).map_err(|_| {
                            GitError::failed_to_read_file(&file_path.to_string_lossy())
                        })?;
                        let file_hash = hash_object::<Blob>(file_content)?;
                        if file_hash != entry.hash {
                            //println!("Conflict in workspace for file: {:?}", file_path);
                            continue;
                        }
                    }

                    let blob = Self::read_blob(gitdir, &entry.hash)?;
                    let content: Vec<u8> = Vec::from(blob);
                    fs::write(&file_path, content).map_err(|_| {
                        println!("Failed to write file");
                        GitError::failed_to_write_file(&file_path.to_string_lossy())
                    })?;
                }
                0o40000 => {
                    // 如果是目录（tree），递归处理子条目
                    if !file_path.exists() {
                        fs::create_dir_all(&file_path).map_err(|_| {
                            GitError::failed_to_write_file(&file_path.to_string_lossy())
                        })?;
                    }
                    let tree = Self::read_tree(gitdir, entry.hash.clone())?;
                    Self::merge_index_into_workspace_for_tree(gitdir, &file_path, &tree)?;
                }
                _ => {
                    return Err(GitError::invalid_command(format!(
                        "Unsupported file mode: {:?}",
                        entry.mode
                    )));
                }
            }
        }
        Ok(())
    }

    fn merge_index_into_workspace_for_tree(gitdir: &PathBuf, base_path: &Path, tree: &Tree) -> Result<()> {
        for entry in &tree.0 {
            let file_path = base_path.join(&entry.path);

            match entry.mode {
                FileMode::Blob => {
                    // 如果是文件（blob），处理文件内容
                    if file_path.exists() {
                        let file_content = fs::read(&file_path).map_err(|_| {
                            GitError::failed_to_read_file(&file_path.to_string_lossy())
                        })?;
                        let file_hash = hash_object::<Blob>(file_content)?;
                        if file_hash != entry.hash {
                            //println!("Conflict in workspace for file: {:?}", file_path);
                            continue;
                        }
                    }
                    let blob = Self::read_blob(gitdir, &entry.hash)?;
                    let content = Vec::<u8>::from(blob);
                    fs::write(&file_path, content).map_err(|_| {
                        GitError::failed_to_write_file(&file_path.to_string_lossy())
                    })?;
                }
                FileMode::Tree => {
                    // 如果是目录（tree），递归处理子条目
                    if !file_path.exists() {
                        fs::create_dir_all(&file_path).map_err(|_| {
                            GitError::failed_to_write_file(&file_path.to_string_lossy())
                        })?;
                    }

                    let sub_tree = Self::read_tree(gitdir, entry.hash.clone())?;
                    Self::merge_index_into_workspace_for_tree(gitdir, &file_path, &sub_tree)?;
                }
                _ => {
                    return Err(GitError::invalid_command(format!(
                        "Unsupported file mode: {:?}",
                        entry.mode
                    )));
                }
            }
        }
        Ok(())
    }


    fn restore_from_index(gitdir: &PathBuf, paths: &[PathBuf]) -> Result<()> {
        let index_path = gitdir.join("index");
        let index = Index::new().read_from_file(&index_path).map_err(|_| {
            GitError::failed_to_read_file(&index_path.to_string_lossy())
        })?;

        for path in paths {
            for entry in &index.entries {
                let entry_path = PathBuf::from(&entry.name);
                if entry_path.starts_with(path) {
                    if entry.mode == 0o40000 {
                        // 如果是目录，创建目录并递归恢复其内容
                        fs::create_dir_all(&entry_path).map_err(|_| {
                            GitError::failed_to_write_file(&entry_path.to_string_lossy())
                        })?;
                        let tree = Self::read_tree(gitdir, entry.hash.clone())?;
                        Self::restore_from_index_for_tree(gitdir, &entry_path, &tree)?;
                    } else if entry.mode == 0o100644 {
                        // 如果是文件，恢复文件内容
                        let blob = Self::read_blob(gitdir, &entry.hash)?;
                        let content = Vec::<u8>::from(blob);
                        fs::write(&entry_path, content).map_err(|_| {
                            GitError::failed_to_write_file(&entry_path.to_string_lossy())
                        })?;
                    }
                    //println!("Restored: {:?}", entry_path);
                }
            }
        }

        Ok(())
    }

    fn restore_from_index_for_tree(gitdir: &PathBuf, base_path: &Path, tree: &Tree) -> Result<()> {
        for entry in &tree.0 {
            let entry_path = base_path.join(&entry.path);
            if entry.mode == FileMode::Tree {
                // 如果是目录，递归处理
                fs::create_dir_all(&entry_path).map_err(|_| {
                    GitError::failed_to_write_file(&entry_path.to_string_lossy())
                })?;
                let sub_tree = Self::read_tree(gitdir, entry.hash.clone())?;
                Self::restore_from_index_for_tree(gitdir, &entry_path, &sub_tree)?;
            } else if entry.mode == FileMode::Blob {
                // 如果是文件，恢复文件内容
                let blob = Self::read_blob(gitdir, &entry.hash)?;
                let content = Vec::<u8>::from(blob);
                fs::write(&entry_path, content).map_err(|_| {
                    GitError::failed_to_write_file(&entry_path.to_string_lossy())
                })?;
            }
            //println!("Restored: {:?}", entry_path);
        }
        Ok(())
    }

    fn restore_from_commit(gitdir: &PathBuf, commit_hash: &str, paths: &[PathBuf]) -> Result<()> {
        let (_, tree) = Self::read_commit(gitdir, commit_hash)?;

        for path in paths {
            Self::restore_path_from_tree(gitdir, path, &tree, PathBuf::new())?;
        }

        Ok(())
    }

    fn restore_path_from_tree(
        gitdir: &PathBuf,
        path: &Path,
        tree: &Tree,
        base_path: PathBuf,
    ) -> Result<()> {
        if let Some(first_component) = path.components().next() {
            let first_component = first_component.as_os_str();
            let remaining_path = path.strip_prefix(first_component).unwrap_or(path);

            for entry in &tree.0 {
                let entry_path = base_path.join(&entry.path);

                if entry.path == first_component {
                    if remaining_path.as_os_str().is_empty() {
                        // 完全匹配路径
                        if entry.mode == FileMode::Tree {
                            // 恢复整个目录
                            fs::create_dir_all(&entry_path).map_err(|_| {
                                GitError::failed_to_write_file(&entry_path.to_string_lossy())
                            })?;
                            let sub_tree = Self::read_tree(gitdir, entry.hash.clone())?;
                            Self::restore_from_commit_for_tree(gitdir, &entry_path, &sub_tree)?;
                        } else if entry.mode == FileMode::Blob {
                            // 恢复文件
                            let blob = Self::read_blob(gitdir, &entry.hash)?;
                            let content = Vec::<u8>::from(blob);
                            fs::write(&entry_path, content).map_err(|_| {
                                GitError::failed_to_write_file(&entry_path.to_string_lossy())
                            })?;
                        }

                        // 更新 index
                        Self::update_index(gitdir, &entry_path, entry)?;
                        //println!("Restored: {:?}", entry_path);
                    } else if entry.mode == FileMode::Tree {
                        // 递归处理子目录
                        let sub_tree = Self::read_tree(gitdir, entry.hash.clone())?;
                        Self::restore_path_from_tree(gitdir, &PathBuf::from(remaining_path), &sub_tree, entry_path)?;
                    }
                }
            }
        }

        Ok(())
    }

    fn restore_from_commit_for_tree(gitdir: &PathBuf, base_path: &Path, tree: &Tree) -> Result<()> {
        for entry in &tree.0 {
            let entry_path = base_path.join(&entry.path);
            if entry.mode == FileMode::Tree {
                // 如果是目录，递归处理
                fs::create_dir_all(&entry_path).map_err(|_| {
                    GitError::failed_to_write_file(&entry_path.to_string_lossy())
                })?;
                let sub_tree = Self::read_tree(gitdir, entry.hash.clone())?;
                Self::restore_from_commit_for_tree(gitdir, &entry_path, &sub_tree)?;
            } else if entry.mode == FileMode::Blob {
                // 如果是文件，恢复文件内容
                let blob = Self::read_blob(gitdir, &entry.hash)?;
                let content = Vec::<u8>::from(blob);
                fs::write(&entry_path, content).map_err(|_| {
                    GitError::failed_to_write_file(&entry_path.to_string_lossy())
                })?;
            }
            //println!("Restored: {:?}", entry_path);
        }
        Ok(())
    }

    fn update_index(gitdir: &Path, entry_path: &Path, entry: &TreeEntry) -> Result<()> {
        let index_path = gitdir.join("index");
        let mut index = Index::new().read_from_file(&index_path).map_err(|_| {
            GitError::failed_to_read_file(&index_path.to_string_lossy())
        })?;

        let existing_entry = index.entries.iter_mut().find(|e| e.name == entry_path.to_string_lossy());
        if let Some(existing_entry) = existing_entry {
            // 如果存在同名条目，更新条目
            existing_entry.mode = entry.mode as u32;
            existing_entry.hash = entry.hash.clone();
        } else {
            // 如果不存在同名条目，新增条目
            index.entries.push(IndexEntry {
                name: entry_path.to_string_lossy().to_string(),
                mode: entry.mode as u32,
                hash: entry.hash.clone(),
            });
        }

        index.write_to_file(&index_path).map_err(|_| {
            GitError::failed_to_write_file(&index_path.to_string_lossy())
        })?;

        Ok(())
    }

}

impl SubCommand for Checkout {
    fn run(&self, gitdir: Result<PathBuf>) -> Result<i32> {
        let gitdir = gitdir?;
        //let mut paths: Vec<PathBuf> = self.paths.iter().map(PathBuf::from).collect();
        let project_root = gitdir.parent().expect("failed to find git dir implementation"). to_path_buf();
        let mut paths: Vec<PathBuf> = self.paths.iter()
            .map(|p| calc_relative_path(&project_root, p))
            .collect::<Result<Vec<_>>>()?; 
        //println!("create_new_branch: {:?}", self.create_new_branch);
        //println!("branch_name_or_commit_hash: {:?}", self.branch_name_or_commit_hash);
        //println!("paths: {:?}", self.paths);
        if let Some(ref commit_or_branch) = self.branch_name_or_commit_hash {
            if commit_or_branch == "HEAD" || commit_or_branch.len() == 40 {
                // println!("checkout from commit {}", commit_or_branch);
                let commit_hash = if commit_or_branch == "HEAD" {
                    read_ref_commit(&gitdir, &read_head_ref(&gitdir)?)?
                } else {
                    commit_or_branch.clone()
                };
                Checkout::restore_from_commit(&gitdir, &commit_hash, &paths)?;
                write_head_commit(&gitdir, &commit_hash)?;
            }
            else{
                //切换分支逻辑
                let heads_dir = gitdir.join("refs/heads");
                let branch_path = heads_dir.join(commit_or_branch);
                if self.create_new_branch {
                    if branch_path.exists() {
                        return Err(GitError::invalid_command(format!("branch '{}' already exists", commit_or_branch)));
                    }
                    let head_ref = read_head_ref(&gitdir)?;
                    let head_ref_path = gitdir.join(&head_ref);
                    if head_ref_path.exists() {
                        let commit_hash = read_ref_commit(&gitdir, &head_ref)?;
                        fs::write(&branch_path, format!("{}\n", commit_hash))
                            .map_err(|_| GitError::failed_to_write_file(&branch_path.to_string_lossy()))?;
                    }
                    write_head_ref(&gitdir, &format!("refs/heads/{}", commit_or_branch))?;
                    return Ok(0);

                }else if !branch_path.exists() {
                    paths.push(PathBuf::from(commit_or_branch));
                }else{
                    let current_ref = read_head_ref(&gitdir)?;
                    //println!("current_ref: {}", current_ref);
                    if format!("refs/heads/{}", commit_or_branch) == current_ref {
                        return Err(GitError::invalid_command(format!("already on branch '{}'", commit_or_branch)));
                    }

                    let current_commit_hash = read_ref_commit(&gitdir, &current_ref)?;

                    let (_, tree) = Self::read_commit(&gitdir, &current_commit_hash)?;


                    let workspace_modified = Self::is_workspace_modified(&gitdir)?;// 检查工作区是否有未暂存的修改
                    let index_modified = Self::is_index_modified(&gitdir, &tree)?;//检查index是否有未commit的修改 
                    //println!("workspace_modified: {}, index_modified: {}", workspace_modified, index_modified);
                    //println!("commit_or_branch: {}", commit_or_branch); 
                    if !workspace_modified && !index_modified {
                        let commit_hash = read_ref_commit(&gitdir, &branch_path.to_string_lossy())?;
                        // 如果没有未暂存或未提交的更改
                        //println!("No uncommitted changes. Switching branch...");

                        write_head_ref(&gitdir, &format!("refs/heads/{}", commit_or_branch))?;
                        let tree_hash = {
                            let commit_path = gitdir.join("objects").join(&commit_hash[0..2]).join(&commit_hash[2..]);
                            let decompressed = decompress_file_bytes(&commit_path)?;
                            Checkout::extract_tree_hash(&decompressed)
                                .ok_or_else(|| GitError::invalid_command(format!("commit {} does not contain a tree", commit_hash)))?
                        };
                        Checkout::restore_workspace(&gitdir, &commit_hash)?;
                        // 使用 ReadTree 恢复索引
                        let read_tree = ReadTree {
                            prefix: None, 
                            tree_hash: tree_hash.clone(),
                        };
                        read_tree.run(Ok(gitdir.clone()))?;
                        return Ok(0);
                    }

                    //println!("Uncommitted changes detected. Attempting to merge changes...");
                    let next_commit_hash = read_ref_commit(&gitdir, &format!("refs/heads/{}", commit_or_branch))?;
                    //println!("branch_name = {}, next_commit_hash = {}", commit_or_branch, next_commit_hash);
                    let (_, nexttree) = Self::read_commit(&gitdir, &next_commit_hash)?;
                    Checkout::merge_tree_into_index_wrapper(&gitdir, &nexttree, Path::new(""))?;
                   // println!("Merged changes from branch '{}'. Now updating workspace...", commit_or_branch);
                    Checkout::merge_index_into_workspace(&gitdir)?;
                    // println!("Switched to branch '{}'", commit_or_branch);
                    write_head_ref(&gitdir, &format!("refs/heads/{}", commit_or_branch))?;
                    //println!("Switched to branch '{}'", commit_or_branch);
                    return Ok(0);
                }
            }

        }
        if !paths.is_empty(){
            //只指定文件路径/目录
            Checkout::restore_from_index(&gitdir, &paths)?;
        } else {
            return Err(GitError::invalid_command("no commit hash or branch name provided".to_string()));
        }
        Ok(0)

    }
}




#[cfg(test)]
mod test {
    use crate::utils::test::{
        shell_spawn, setup_test_git_dir,
        tempdir,
    };
    use super::*;

    #[test]
    fn test_checkout_single_file() {
        let repo = setup_test_git_dir();

        // 创建文件并提交
        let file_path = repo.path().join("foo.txt");
        std::fs::write(&file_path, "hello").unwrap();
        shell_spawn(&["git", "-C", repo.path().to_str().unwrap(), "add", "foo.txt"]).unwrap();
        shell_spawn(&["git", "-C", repo.path().to_str().unwrap(), "commit", "-m", "add foo"]).unwrap();

        // 修改文件内容
        std::fs::write(&file_path, "changed").unwrap();

        // 执行 checkout 恢复文件
        shell_spawn(&["git", "-C", repo.path().to_str().unwrap(), "checkout", "foo.txt"]).unwrap();

        // 验证文件内容是否恢复
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "hello");
    }

    #[test]
    fn test_checkout_entire_directory() {
        let repo = setup_test_git_dir();

        // 创建目录和文件并提交
        let dir_path = repo.path().join("dir");
        std::fs::create_dir_all(&dir_path).unwrap();
        std::fs::write(dir_path.join("a.txt"), "A").unwrap();
        std::fs::write(dir_path.join("b.txt"), "B").unwrap();
        shell_spawn(&["git", "-C", repo.path().to_str().unwrap(), "add", "."]).unwrap();
        shell_spawn(&["git", "-C", repo.path().to_str().unwrap(), "commit", "-m", "add dir"]).unwrap();

        // 修改文件内容
        std::fs::write(dir_path.join("a.txt"), "X").unwrap();
        std::fs::write(dir_path.join("b.txt"), "Y").unwrap();

        // 执行 checkout 恢复目录
        shell_spawn(&["git", "-C", repo.path().to_str().unwrap(), "checkout", "dir"]).unwrap();

        // 验证文件内容是否恢复
        let content_a = std::fs::read_to_string(dir_path.join("a.txt")).unwrap();
        let content_b = std::fs::read_to_string(dir_path.join("b.txt")).unwrap();
        assert_eq!(content_a, "A");
        assert_eq!(content_b, "B");
    }

    #[test]
    fn test_checkout_file_from_commit() {
        let repo = setup_test_git_dir();

        // 创建文件并提交两次
        let file_path = repo.path().join("foo.txt");
        std::fs::write(&file_path, "v1").unwrap();
        shell_spawn(&["git", "-C", repo.path().to_str().unwrap(), "add", "foo.txt"]).unwrap();
        shell_spawn(&["git", "-C", repo.path().to_str().unwrap(), "commit", "-m", "c1"]).unwrap();
        std::fs::write(&file_path, "v2").unwrap();
        shell_spawn(&["git", "-C", repo.path().to_str().unwrap(), "add", "foo.txt"]).unwrap();
        shell_spawn(&["git", "-C", repo.path().to_str().unwrap(), "commit", "-m", "c2"]).unwrap();

        // 获取第一个提交的哈希
        let commit1 = shell_spawn(&["git", "-C", repo.path().to_str().unwrap(), "rev-parse", "HEAD~1"])
            .unwrap()
            .trim()
            .to_string();

        // 修改文件内容
        std::fs::write(&file_path, "changed").unwrap();

        // 执行 checkout 恢复文件到第一个提交的状态
        shell_spawn(&["git", "-C", repo.path().to_str().unwrap(), "checkout", &commit1, "--", "foo.txt"]).unwrap();

        // 验证文件内容是否恢复
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "v1");
    }

    #[test]
    fn test_checkout_directory_from_commit() {
        let repo = setup_test_git_dir();

        // 创建目录和文件并提交两次
        let dir_path = repo.path().join("dir");
        std::fs::create_dir_all(&dir_path).unwrap();
        std::fs::write(dir_path.join("a.txt"), "A1").unwrap();
        shell_spawn(&["git", "-C", repo.path().to_str().unwrap(), "add", "."]).unwrap();
        shell_spawn(&["git", "-C", repo.path().to_str().unwrap(), "commit", "-m", "c1"]).unwrap();
        std::fs::write(dir_path.join("a.txt"), "A2").unwrap();
        shell_spawn(&["git", "-C", repo.path().to_str().unwrap(), "add", "."]).unwrap();
        shell_spawn(&["git", "-C", repo.path().to_str().unwrap(), "commit", "-m", "c2"]).unwrap();

        // 获取第一个提交的哈希
        let commit1 = shell_spawn(&["git", "-C", repo.path().to_str().unwrap(), "rev-parse", "HEAD~1"])
            .unwrap()
            .trim()
            .to_string();

        // 修改文件内容
        std::fs::write(dir_path.join("a.txt"), "changed").unwrap();

        // 执行 checkout 恢复目录到第一个提交的状态
        shell_spawn(&["git", "-C", repo.path().to_str().unwrap(), "checkout", &commit1, "--", "dir"]).unwrap();

        // 验证文件内容是否恢复
        let content_a = std::fs::read_to_string(dir_path.join("a.txt")).unwrap();
        assert_eq!(content_a, "A1");
    }

    #[test]
    fn test_ppt_checkout() -> Result<()> {
        let temp_dir = tempdir()?;
        let curr_dir = std::env::current_dir().unwrap();

        println!("{}", shell_spawn(&["mkdir", temp_dir.path().join("tests").to_str().unwrap()])?);
        println!("{}", shell_spawn(&["cp", curr_dir.join("target/debug/git").to_str().unwrap(), temp_dir.path().join("tests/rust-git").to_str().unwrap()])?);
        println!("{}", shell_spawn(&["chmod", "a+x", temp_dir.path().join("tests").join("rust-git").to_str().unwrap()])?);

        std::env::set_current_dir(&temp_dir)?;
        println!("output = {}", shell_spawn(&[curr_dir.join("tests/test_branch_checkout").to_str().unwrap()])?);
        Ok(())
    }
}
