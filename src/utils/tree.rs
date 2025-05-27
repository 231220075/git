use std::{
    fmt,
    cmp::{
        PartialOrd,
        Ord,
        PartialEq,
        Eq,
        Ordering,
    },
    result,
    ops::{
        Deref,
        DerefMut,
    },
    convert::{
        TryFrom,
    },
    error::Error,
    path::PathBuf,
    iter::Iterator,
};

use hex;
use nom::{
    Parser,
    bytes::complete::{tag, take, take_until},
    number::complete::be_u32,
    character::complete::{alpha1, digit1, space1, u32},
    sequence::{
        terminated,
        preceded,
        separated_pair,
    },
    multi::{
        many0,
        many1,
    },
    combinator::{
        map, map_res, eof, value,
    },
    IResult,
};
use hex::encode;

use crate::utils::{
    objtype::{
        Obj,
        ObjType,
        parse_meta,
    },
    fs::{
        read_obj,
        read_object,
    },
    error::{
        GitError,
        Result
    },
    index::{
        Index,
        IndexEntry,
    },
};


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileMode {
    Exec     = 0o100755,
    Blob     = 0o100644,
    Tree     = 0o040000,
    Commit   = 0o160000,
    Symbolic = 0o120000,
}

impl fmt::Display for FileMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}",
            match self {
                FileMode::Blob | FileMode::Exec => "blob",
                FileMode::Tree => "tree",
                FileMode::Commit => "commit",
                FileMode::Symbolic => "symbolic",
            })
    }
}

impl TryFrom<u32> for FileMode {
    type Error = Box<dyn Error>;

    fn try_from(integer: u32) -> result::Result<Self, Self::Error> {
        match integer {
            0o100755 => Ok(FileMode::Exec),
            0o100644 => Ok(FileMode::Blob),
            0o40000  => Ok(FileMode::Tree),
            0o160000 => Ok(FileMode::Commit),
            0o120000 => Ok(FileMode::Symbolic),
            other => Err(GitError::invalid_filemode(other.to_string()))
        }
    }
}

impl From<FileMode> for &'static str {

    fn from(mode: FileMode) -> &'static str {
        match mode {
            FileMode::Exec     => "100755",
            FileMode::Blob     => "100644",
            FileMode::Tree     => "040000",
            FileMode::Commit   => "160000",
            FileMode::Symbolic => "120000",
        }
    }
}

impl TryFrom<&[u8]> for FileMode {
    type Error = Box<dyn Error>;

    fn try_from(bytes: &[u8]) -> result::Result<Self, Self::Error> {
        let mode = String::from_utf8(bytes.to_vec())?;
        match mode.as_str() {
            "100755" => Ok(FileMode::Exec),
            "100644" => Ok(FileMode::Blob),
            "40000"  => Ok(FileMode::Tree),
            "160000" => Ok(FileMode::Commit),
            "120000" => Ok(FileMode::Symbolic),
            other    => Err(GitError::invalid_filemode(other.to_string()))
        }
    }
}

#[derive(Clone, Debug)]
pub struct TreeEntry {
    pub mode: FileMode, //
    pub hash: String,   // 40 length
    pub path: PathBuf,  // relative to git dir
}

type EntryPrototype<'a> = (&'a[u8], &'a[u8], &'a[u8]);
impl<'a> TryFrom<EntryPrototype<'a>> for TreeEntry {
    type Error = Box<dyn Error>;

    fn try_from(enp: EntryPrototype) -> result::Result<Self, Self::Error> {
        let mode = enp.0.try_into()?;
        let path = PathBuf::from(&String::from_utf8(enp.1.to_vec())?);
        let hash = encode(enp.2);
        Ok(TreeEntry {
            mode,
            hash,
            path,
        })
    }
}

impl TreeEntry {
    fn parse_from_bytes(bytes: &[u8]) -> IResult<&[u8], EntryPrototype> {
        let parse_mode = terminated(take_until(" "), tag(" "));
        let parse_path = terminated(take_until("\0"), tag("\0"));
        let parse_hash = take(20usize);

        (
            parse_mode,
            parse_path,
            parse_hash,
        ).parse(bytes)
    }

    fn into_iter(self) -> impl Iterator<Item = u8> {
        let mode: &str = self.mode.into();
        let hash = hex::decode(&self.hash).unwrap();
        let path = self.path.to_str().unwrap().as_bytes().to_vec();

        mode.to_string()
            .into_bytes()
            .into_iter()
            .chain(b" ".iter().cloned())
            .chain(path)
            .chain(b"\0".iter().cloned())
            .chain(hash)
    }

    fn into_iter_flatten(self, gitdir: PathBuf) -> Result<Vec<Self>> {
        let obj = read_obj(gitdir.clone(), &self.hash)?;
        // println!("self = {}", self);
        match obj {
            Obj::B(_) => Ok(vec![self]),
            Obj::T(tree) => Ok(tree.0
                .into_iter()
                .map(|entry|entry.into_iter_flatten(gitdir.clone()))
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .map(|TreeEntry{mode, hash, path}| TreeEntry {
                    mode,
                    hash,
                    path: self.path.join(path)
                })
                .collect::<Vec<_>>()),
            Obj::C(cmt) => Err(GitError::invalid_commit(&format!("commit object {cmt} in tree object! your git repo is totaly fucked up!")))
        }
    }
}


impl From<IndexEntry> for TreeEntry {
    fn from(entry: IndexEntry) -> Self {
        Self {
            mode: entry.mode.try_into().unwrap(),
            hash: entry.hash,
            path: PathBuf::from(entry.name),
        }
    }
}

impl TryFrom<&[u8]> for TreeEntry {
    type Error = Box<dyn Error>;

    fn try_from(bytes: &[u8]) -> result::Result<Self, Self::Error> {
        // [mode] space [path] 0x00 [sha-1]

        let (_, (modebytes, hashbytes, pathbytes)) = TreeEntry::parse_from_bytes(bytes)
                                                                                    .map_err(GitError::invalid_entry)?;

        let mode = modebytes.try_into()?;
        let path = PathBuf::from(String::from_utf8(pathbytes.to_vec())?);
        let hash = String::from_utf8(hashbytes.to_vec())?;
        Ok(TreeEntry {
            mode,
            hash,
            path,
        })
    }
}

impl PartialEq for TreeEntry {
    fn eq(&self, other: &Self) -> bool {
        self.path.eq(&other.path)
    }
}

impl Eq for TreeEntry {
}

impl PartialOrd for TreeEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.path.cmp(&other.path))
    }
}

impl Ord for TreeEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.path.cmp(&other.path)
    }
}


impl fmt::Display for TreeEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:06o} {} {}\t{}", self.mode as u32, self.mode, self.hash.clone(), self.path.display())
    }
}

pub struct Tree(pub Vec<TreeEntry>);

impl Tree {
    pub fn into_iter_flatten(self, gitdir: PathBuf) -> Result<impl IntoIterator<Item = TreeEntry>> {
        Ok(self.0.into_iter()
            .map(|en|en.into_iter_flatten(gitdir.clone()))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten())
    }
}

impl TryFrom<Vec<u8>> for Tree {
    type Error = Box<dyn Error>;

    fn try_from(bytes: Vec<u8>) -> result::Result<Self, Self::Error> {
        let (entrys, _) = parse_meta(bytes.as_slice()).map_err(GitError::invalid_tree)?;

        let (_, entrys_str) = terminated(many0(TreeEntry::parse_from_bytes), eof)
            .parse(entrys)
            .map_err(GitError::invalid_entry)?;
        let entrys = entrys_str
            .into_iter()
            .map(|prototype| prototype.try_into())
            .collect::<Result<Vec<TreeEntry>>>()?;

        Ok(Tree(entrys))
    }
}

impl From<Tree> for Vec<u8> {
    fn from(val: Tree) -> Vec<u8> {
        val.0
            .into_iter()
            .flat_map(|x|x.into_iter())
            .collect()
    }
}


impl ObjType for Tree {
    const VALUE: &'static str = "tree";
    const MODE: u32 = 0o040000;
}

impl fmt::Display for Tree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.0.iter().map(|x|x.to_string()).collect::<Vec<_>>().join("\n"))
    }
}

impl TryFrom<Obj> for Tree {
    type Error = Box<dyn Error>;

    fn try_from(obj: Obj) -> Result<Tree> {
        match obj {
            Obj::T(tree) => Ok(tree),
            _ => Err(GitError::not_a_ttree("think twice before do it!")),
        }
    }
}

impl Deref for Tree {
    type Target = Vec<TreeEntry>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Tree {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Index> for Tree {
    fn from(index: Index) -> Self {
        Self(
            index.entries
            .into_iter()
            .map(|x|x.into())
            .collect::<Vec<_>>()
        )
    }
}
