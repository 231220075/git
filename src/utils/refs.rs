use std::path::Path;
use std::fs;
use crate::{
    utils::{
        commit::Commit,
        fs::read_file_as_bytes,
        objtype::Obj,
    },
    GitError, Result
};

/// read from/write to .git/HEAD
/// content may look like ref: refs/heads/branch
pub fn read_head_ref(gitdir: &Path) -> Result<String> {
    let head_path = gitdir.join("HEAD");
    let content = fs::read_to_string(&head_path)
        .map_err(|_| GitError::FileNotFound(head_path.display().to_string()))?;
    // 解析 ref: refs/heads/main
    if let Some(rest) = content.strip_prefix("ref: ") {
        Ok(rest.trim().to_string())
    } else {
        Err(GitError::detached_branch(content))
    }
}

pub fn write_head_ref(gitdir: &Path, ref_path: &str) -> Result<()> {
    let head_file = gitdir.join("HEAD");
    fs::write(&head_file, format!("ref: {}\n", ref_path))
        .map_err(|_| GitError::failed_to_write_file(&head_file.to_string_lossy()))?;
    Ok(())
}

pub fn write_head_commit(gitdir: &Path, hash: &str) -> Result<()> {
    let head_file = gitdir.join("HEAD");
    fs::write(&head_file, format!("{}\n", hash))
        .map_err(|_| GitError::failed_to_write_file(&head_file.to_string_lossy()))?;
    Ok(())
}

pub fn read_head_commit(gitdir: &Path) -> Result<String> {
    let head_path = gitdir.join("HEAD");
    let content = fs::read_to_string(&head_path)
        .map_err(|_| GitError::FileNotFound(head_path.display().to_string()))?;
    Ok(content.trim().to_string())
}

/// read from / write to .git/{refname}
/// content is 20 bytes commit hash, such as fbb2fa502d19588f97190d8c89643aad3e533bb8
pub fn read_ref_commit(gitdir: &Path, refname: &str) -> Result<String> {
    let ref_path = gitdir.join(refname);
    let content = fs::read_to_string(&ref_path)
        .map_err(|_| GitError::FileNotFound(format!("不存在 {} 这个分支", ref_path.file_name().unwrap().to_str().unwrap())))?;
    Ok(content.trim().to_string())
}

pub fn write_ref_commit(gitdir: &Path, ref_path: &str, hash: &str) -> Result<()> {
    let ref_file = gitdir.join(ref_path);
    fs::write(&ref_file, format!("{}\n", hash))
        .map_err(|_| GitError::failed_to_write_file(&ref_file.to_string_lossy()))?;
    Ok(())
}

pub fn read_branch_commit(gitdir: &Path, branch: &str) -> Result<String> {
    read_ref_commit(gitdir, &format!("refs/heads/{}", branch))
}

pub fn write_branch_commit(gitdir: &Path, branch: &str, hash: &str) -> Result<()> {
    write_ref_commit(gitdir, &format!("refs/heads/{}", branch), hash)
}

pub fn head_to_hash(gitdir: &Path) -> Result<String> {
    let head_ref = read_head_ref(gitdir)?;
    read_ref_commit(gitdir, &head_ref)
}

