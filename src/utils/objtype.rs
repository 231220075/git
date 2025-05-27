use std::{
    fmt,
    result,
    convert::TryFrom,
    error::Error,
    path::PathBuf,
    fmt::Display,
};

use crate::utils::{
    blob::Blob,
    tree::Tree,
    commit::Commit,
    error::{
        GitError,
        Result
    },
};

use nom::{
    Parser,
    bytes::complete::{tag, take, take_until},
    number::complete::be_u32,
    character::complete::{digit1, space1, u32, alpha1},
    sequence::{
        terminated,
        preceded,
        separated_pair,
    },
    branch::alt,
    combinator::{
        map, map_res, eof, value
    },
    IResult,
};


pub trait ObjType: TryFrom<Vec<u8>> + Into<Vec<u8>> + TryFrom<Obj> {
    const VALUE: &'static str;
    const MODE: u32;
}

pub enum Obj {
    B(Blob),
    T(Tree),
    C(Commit),
}

impl Obj {
    pub fn get_type(&self) -> &'static str {
        match self {
            Obj::B(_) => Blob::VALUE,
            Obj::T(_) => Tree::VALUE,
            Obj::C(_) => Commit::VALUE,
        }
    }

}

impl TryFrom<Vec<u8>> for Obj {
    type Error = Box<dyn Error>;

    fn try_from(bytes: Vec<u8>) -> result::Result<Self, Self::Error> {
        let index = bytes.iter().position(|&b| b == b'\0')
            .ok_or(GitError::invalid_filemode(String::from_utf8_lossy(&bytes).into_owned()))?;

        let (_, (t, _)) = parse_meta(&bytes[..=index]).map_err(GitError::invalid_tree)?;
        match t {
            b"blob"   => Ok(Obj::B(bytes.to_vec().try_into()?)),
            b"tree"   => Ok(Obj::T(bytes.to_vec().try_into()?)),
            b"commit" => Ok(Obj:: C(bytes.to_vec().try_into()?)),
            _        => Err(GitError::invalid_filemode(String::from_utf8_lossy(&bytes).into_owned()))
        }
    }
}

impl From<Obj> for Vec<u8> {
    fn from(val: Obj) -> Vec<u8> {
        match val {
            Obj::B(b) => b.into(),
            Obj::T(t) => t.into(),
            Obj::C(c) => c.into(),
        }
    }
}

impl fmt::Display for Obj {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Obj::B(b) => {
                b.fmt(f)
            },
            Obj::T(b) => b.fmt(f),
            Obj::C(b) => b.fmt(f),
        }
    }
}

pub fn parse_meta(bytes: &[u8]) -> IResult<&[u8], (&[u8], &[u8])> {
    terminated(separated_pair(
            alt((tag("blob"), tag("tree"), tag("commit"))),
            tag(" "),
            digit1,
        ),
        tag("\0")
    ).parse(bytes)
}
