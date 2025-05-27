use std::fmt;
use std::error::Error;
use std::path::Path;
use GitError::{InvalidCommand, FileNotFound, NoSubCommand};

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Debug, Clone)]
pub enum GitError {
    InvalidCommand(String),
    InvalidHash(String),
    InvalidBlob(String),
    InvalidFileMode(String),
    InvalidEntry(String),
    InvalidTree(String),
    InvalidCommit(String),
    InvaildPathEncoding(String),
    FileNotFound(String),
    InvalidObj(String),
    NoPermision(String),
    NotARepoFile(String),
    NotABBlob(String),
    NotATTree(String),
    NotACCommit(String),
    DetachedBranch(String),
    FailedToReadFile(String),
    FailedToWriteFile(String),
    FailedToRmoveFile(String),
    BrokenCommitHistory(String),
    MergeConflict(String),
    NoSameAncestor(String),
    NoSubCommand,
    NotInGitRepo,
}

impl GitError {
    pub fn no_same_ancestor(msg: String) -> Box::<dyn Error> {
        Box::new(
            Self::MergeConflict(msg)
        )
    }
    pub fn merge_conflict(msg: String) -> Box::<dyn Error> {
        Box::new(
            Self::MergeConflict(msg)
        )
    }
    pub fn not_a_ccommit(msg: &str) -> Box::<dyn Error> {
        Box::new(
            Self::NotACCommit(msg.to_string())
        )
    }
    pub fn not_a_ttree(msg: &str) -> Box::<dyn Error> {
        Box::new(
            Self::NotATTree(msg.to_string())
        )
    }
    pub fn not_a_bblob(msg: &str) -> Box::<dyn Error> {
        Box::new(
            Self::NotABBlob(msg.to_string())
        )
    }
    pub fn invalid_obj(msg: String) -> Box::<dyn Error> {
        Box::new(
            Self::InvalidObj(msg)
        )
    }
    pub fn invalid_tree(err: impl Error) -> Box::<dyn Error> {
        Box::new(
            Self::InvalidEntry(err.to_string())
        )
    }
    pub fn invalid_entry(err: impl Error) -> Box::<dyn Error> {
        Box::new(
            Self::InvalidEntry(err.to_string())
        )
    }
    pub fn invalid_filemode(mode_str: String) -> Box::<dyn Error> {
        Box::new(
            Self::InvalidFileMode(mode_str)
        )
    }

    pub fn no_permision(err: impl Error) -> Box::<dyn Error> {
        Box::new(
            Self::NoPermision(err.to_string())
        )
    }

    pub fn not_a_repofile<P: AsRef<Path>>(file: P) -> Box::<dyn Error>
    {
        Box::new(
            Self::NotARepoFile(file.as_ref()
                .to_str()
                .unwrap()
                .to_string())
        )
    }

    pub fn no_subcommand() -> Box::<dyn Error> {
        Box::new(
            Self::NoSubCommand
        )
    }

    pub fn file_notfound(msg: String) -> Box<dyn Error> {
        Box::new(
            Self::FileNotFound(msg)
        )
    }

    pub fn invalid_command(msg: String) -> Box<dyn Error> {
        Box::new(
            Self::InvalidCommand(msg.to_string())
        )
    }

    pub fn not_in_gitrepo() -> Box<dyn Error> {
        Box::new(
            Self::NotInGitRepo
        )
    }

    pub fn invalid_blob(path: &str) -> Box<dyn Error> {
        Box::new(
            Self::InvalidBlob(format!("invlaid blob format: {}", path))
        )
    }

    pub fn invalid_hash(hash: &str) -> Box<dyn Error> {
        Box::new(
            Self::InvalidHash(format!("expect hash code of length 40 but got {} of length {}", hash, hash.len()))
        )
    }

    pub fn invaild_path_encoding(path: &str) -> Box<dyn Error>{
        Box::new(
            Self::InvaildPathEncoding(format!("invlaid path encoding: {}", path))
        )
    }

    pub fn invalid_commit(path: &str) -> Box<dyn Error> {
        Box::new(
            Self::InvalidCommit(format!("invlaid commit: {}", path))
        )
    }

    pub fn failed_to_read_file(path: &str) -> Box<dyn Error> {
        Box::new(
            Self::FailedToReadFile(format!("failed to read file: {}", path))
        )
    }

    pub fn failed_to_write_file(path: &str) -> Box<dyn Error> {
        Box::new(
            Self::FailedToWriteFile(format!("failed to write file: {}", path))
        )
    }

    pub fn failed_to_remove_file(msg: String) -> Box<dyn Error> {
        Box::new(
            Self::FailedToWriteFile(msg)
        )
    }

    pub fn detached_branch(hash: String) -> Box<dyn Error> {
        Box::new(
            Self::DetachedBranch(hash)
        )
    }

    pub fn broken_commit_history(hash: String) -> Box<dyn Error> {
        Box::new(
            Self::DetachedBranch(hash)
        )
    }
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InvalidCommand(cmd) => write!(f, "Invalid command: {}", cmd),
            FileNotFound(file)  => write!(f, "File not found: {}",  file),
            GitError::InvalidHash(hash) => write!(f, "Invalid hash: {}", hash),
            GitError::NoSubCommand => write!(f, "no sub command"),
            GitError::NotInGitRepo => write!(f, "not in a git repository"),
            GitError::InvalidBlob(msg) => write!(f, "{}", msg),
            GitError::InvalidFileMode(mode) => write!(f, "Invalid FileMode {}", mode),
            GitError::InvalidEntry(msg) => write!(f, "Invalid Entry {}", msg),
            GitError::InvalidTree(msg) => write!(f, "Invalid Tree {}", msg),
            GitError::InvalidObj(msg) => write!(f, "Invalid Obj {}", msg),
            GitError::InvalidCommit(msg) => write!(f, "{}", msg),
            GitError::InvaildPathEncoding(path) => write!(f, "invalid path encoding: {}", path),
            GitError::NoPermision(msg) => write!(f, "no access permission: {}", msg),
            GitError::NotARepoFile(path) => write!(f, "found a file not in git repo {}", path),
            GitError::FailedToReadFile(path) => write!(f, "failed to read file: {}", path),
            GitError::FailedToWriteFile(path) => write!(f, "failed to write file: {}", path),
            GitError::FailedToRmoveFile(msg) => write!(f, "{}", msg),
            GitError::DetachedBranch(hash) => write!(f, "detached head {} current branch not found", hash),
            GitError::BrokenCommitHistory(hash) => write!(f, "{} commit is broken", hash),
            GitError::NotABBlob(msg) => write!(f, "debug Error, should not happen in release: {}", msg),
            GitError::NotATTree(msg) => write!(f, "debug Error, should not happen in release: {}", msg),
            GitError::NotACCommit(msg) => write!(f, "debug Error, should not happen in release: {}", msg),
            GitError::MergeConflict(msg) => write!(f, "{}", msg),
            GitError::NoSameAncestor(msg) => write!(f, "{}", msg),
        }
    }
}

impl Error for GitError {}
