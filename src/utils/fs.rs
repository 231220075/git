use std::{
    error::Error,
    env::current_dir,
    io::{BufReader, Read},
    fs::{read, File},
    path::{PathBuf, Path},
};

use crate::{
    GitError,
    Result,
};

use super::{
    hash::hash_object,
    zlib::{
        compress_object,
        decompress_file_as_bytes,
    },
    objtype::{
        Obj,
        ObjType,
    },
    index:: {
        IndexEntry,
        Index,
    },
    tree::FileMode,
};

use std::fs;
use std::os::unix::fs::PermissionsExt; // 用于操作 Unix 文件权限

fn is_executable(file_path: impl AsRef<Path>) -> Result<bool> {
    let metadata = fs::metadata(file_path)
        .map_err(GitError::no_permision)?;

    let permissions = metadata.permissions();

    let mode = permissions.mode();
    Ok(mode & 0o111 != 0) // 检查用户、组或其他用户的可执行位是否被设置
}


/*  check the whether s exists in git's objects directory  */
pub fn obj_to_pathbuf(s: &str) -> std::result::Result<PathBuf, String> {
    if s.len() != 40 {
        Err(format!("{} 长度不等于40，实际长度: {}", s, s.len()))
    }
    else {
        let (first, second) = s.split_at(2);
        let mut path = PathBuf::new();
        path.extend(["objects", first, second]);
        Ok(path)
    }
}

pub fn to_pathbuf(slice: &str) -> std::result::Result<PathBuf, String> {
    Ok(PathBuf::from(slice))
}

pub fn read_file_as_bytes<T>(file_path: &T) -> Result<Vec<u8>>
where T: AsRef<Path>
{
    // 使用 fs::read 读取文件内容为字节数组
    Ok(read(file_path)?)
}

pub fn read_file_as_reader<T>(file_path: &T) -> Result<impl Read>
where T: AsRef<Path>
{
    let file = File::open(file_path)?;
    Ok(BufReader::new(file))
}


fn search_dir(mut path: PathBuf, target: &str) -> Result<PathBuf>
{
    path.push(target);
    if path.exists() && path.is_dir() {
        Ok(path)
    }
    else if !path.pop() || !path.pop() {
        Err(GitError::not_in_gitrepo())
    }
    else {
        search_dir(path, target)
    }
}

pub fn get_git_dir() -> Result<PathBuf> {
    search_git_dir(current_dir().unwrap())
}

pub fn search_git_dir<T>(path: T) -> Result<PathBuf>
where T: AsRef<Path>
{
    search_dir(PathBuf::from(path.as_ref()), ".git")
}

pub fn write_object<T: ObjType>(mut gitdir: PathBuf, content: Vec<u8>) -> Result<String> {
    let commit_hash = hash_object::<T>(content.clone())?;

    gitdir.extend(["objects", &commit_hash[0..2], &commit_hash[2..]]);

    std::fs::create_dir_all(gitdir.parent().unwrap()).map_err(GitError::no_permision)?;
    std::fs::write(
        &gitdir,
    compress_object::<T>(content)?).map_err(GitError::no_permision)?;

    Ok(commit_hash)
}

pub fn read_obj(mut gitdir: PathBuf, hash: &str) -> Result<Obj> {
    gitdir.extend(["objects", &hash[0..2], &hash[2..]]);
    let bytes = decompress_file_as_bytes(&gitdir)?;
    // println!("read {}", gitdir.display());
    // println!("string = {}", String::from_utf8_lossy(&bytes).to_owned());
    bytes.try_into()
}

pub fn read_object<T>(gitdir: PathBuf, hash: &str) -> Result<T>
where
    T: ObjType + TryFrom<Obj, Error=Box<dyn Error>>
{
    let obj = read_obj(gitdir, hash)
        .map_err(|e|GitError::invalid_obj(format!("fail to read {} object {}\n", T::VALUE, hash) + &e.to_string()))?;
    obj.try_into()
}

pub fn add_object<T>(gitdir: PathBuf, path: impl AsRef<Path>) -> Result<IndexEntry>
where
    T: ObjType,
{
    let project_root = gitdir.parent().expect("find git implementation fail").to_path_buf();
    let mode = if is_executable(project_root.join(&path))? { FileMode::Exec as u32 } else { T::MODE };
    let hash = write_object::<T>(gitdir, read_file_as_bytes(&project_root.join(&path))?)?;
    let path = String::from(path.as_ref().to_str().unwrap());
    Ok(IndexEntry {
        mode,
        hash,
        name: path,
    })
}


pub fn walk<P>(path: P) -> Result<impl IntoIterator<Item = PathBuf>>
where
    P: AsRef<Path>
{
    if path.as_ref().is_dir() {
        let pathbufs = path.as_ref()
            .read_dir().map_err(GitError::no_permision)?
            .map(|x| x.map(|x|x.path()).map_err(GitError::no_permision))
            .collect::<Result<Vec<_>>>()?;

        let files = pathbufs.iter()
            .filter(|x|x.is_file())
            .cloned()
            .collect::<Vec<_>>();

        let iter_dirs = pathbufs.into_iter()
            .filter(|x|x.is_dir())
            .filter(|x| {
                !x.strip_prefix(&path).unwrap().starts_with(".git")
            })
            .map(walk)
            .collect::<Result<Vec<_>>>()
            .map(|x|x.into_iter().flatten());

        iter_dirs
            .map(|x|x.into_iter().chain(files).collect::<Vec<_>>())

    }
    else {
        Ok([path.as_ref().to_path_buf()].to_vec())
    }
}

/// assert path is child or son of dir and return path's relative path of dir
pub fn calc_relative_path<P, M>(dir: P, path: M) -> Result<PathBuf>
where
    P: AsRef<Path>,
    M: AsRef<Path>,
{
    let dir_path = dir.as_ref().to_path_buf();
    let abs = dir_path
        .join(path.as_ref())
        .canonicalize()
        .map_err(|x|GitError::not_a_repofile(path.as_ref().to_path_buf().display().to_string() + " " + &x.to_string()))?;

    if dir.as_ref() == abs {
        Ok(PathBuf::from("."))
    }
    else if dir_path.join(&abs) == abs {
        abs.strip_prefix(dir.as_ref())
            .map(|x|x.to_path_buf())
            .map_err(|x|GitError::not_a_repofile(abs.display().to_string() + " " + &x.to_string()))
    }
    else {
        Err(GitError::not_a_repofile(path.as_ref()))
    }
}


