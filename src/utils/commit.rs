use std::{
    fmt,
    result,
    convert::TryFrom,
    error::Error,
    path::PathBuf
};

use crate::utils:: {
    objtype::{
        Obj,
        ObjType,
        parse_meta,
    },
    error::{
        GitError,
        Result
    },
};

use nom::{
    Parser,
    multi::many0,
    bytes::complete::{
        tag, take, take_until, take_while,
    },
    number::complete::be_u32,
    character::complete::{digit1, space1, u32, alpha1, },
    sequence::{
        terminated,
        preceded,
        separated_pair,
    },
    branch::alt,
    combinator::{
        map, map_res, eof, value, opt
    },
    IResult,
};


pub struct Commit {
    pub tree_hash: String,
    pub parent_hash: Vec<String>,
    pub author: String,
    pub committer: String,
    pub message: String,
}

type CommitPrototype<'a> = (&'a[u8], Vec<&'a[u8]>, &'a[u8], &'a[u8], &'a[u8]);
impl Commit {
    fn parse_from_bytes<'a>(bytes: &'a[u8]) -> IResult<&'a [u8], CommitPrototype<'a>> {
        let parse_tree = terminated(preceded(tag("tree "),take_until("\n")), tag("\n"));
        let parse_parent = many0(terminated(preceded(tag("parent "),take_until("\n")), tag("\n")));
        let parse_author = terminated(preceded(tag("author "),take_until("\n")), tag("\n"));
        let parse_committer = terminated(preceded(tag("committer "),take_until("\n")), tag("\n"));
        let parse_messages = preceded(tag("\n"), take_while(|_|true));
        (
            parse_tree,
            parse_parent,
            parse_author,
            parse_committer,
            parse_messages,
        ).parse(bytes)
    }
}

impl ObjType for Commit {
    const VALUE: &'static str = "commit";
    const MODE: u32 = 0o160000;
}

impl TryFrom<Vec<u8>> for Commit {
    type Error = Box<dyn Error>;

    fn try_from(bytes: Vec<u8>) -> result::Result<Self, Self::Error> {
        let ( _,
                (_, (tree_hash, parent_hash, author, committer, message))) = (
                parse_meta,
                Commit::parse_from_bytes
            ).parse(&bytes)
            .map_err(|e|GitError::invalid_commit(&e.to_string()))?;

        let parent_hash = parent_hash.into_iter()
            .map(|x|x.to_vec())
            .map(|v|String::from_utf8(v).map_err(|e|GitError::invalid_commit(&e.to_string())))
            .collect::<Result<Vec<_>>>()?;
        Ok(Commit {
            tree_hash:   String::from_utf8(tree_hash.to_vec())?,
            parent_hash,
            author:      String::from_utf8(author.to_vec())?,
            committer:   String::from_utf8(committer.to_vec())?,
            message:     String::from_utf8(message.to_vec())?,
        })
    }
}

impl From<Commit> for Vec<u8> {
    fn from(commit: Commit) -> Vec<u8> {
        let parent_line = commit.parent_hash.into_iter()
            .map(|hash| format!("parent {}\n", hash))
            .collect::<String>();
        // println!("parent_line = {}", parent_line);
        format!("tree {}\n{}\
                author {}\n\
                committer {}\n\
                \n\
                {}",
            commit.tree_hash,
            parent_line,
            commit.author,
            commit.committer,
            if commit.message.ends_with("\n") {commit.message} else {format!("{}\n", commit.message)},
        ).into_bytes()
    }
}

impl fmt::Display for Commit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parent_line = self.parent_hash.iter()
            .map(|hash| format!("parent {}\n", hash))
            .collect::<String>();
        // println!("parent_line = {}", parent_line);
        write!(f, "tree {}\n{}\
                   author {}\n\
                   committer {}\n\
                   \n\
                   {}",
                self.tree_hash,
                parent_line,
                self.author,
                self.committer,
                self.message,
        )
    }
}

impl TryFrom<Obj> for Commit {
    type Error = Box<dyn Error>;

    fn try_from(obj: Obj) -> Result<Commit> {
        match obj {
            Obj::C(commit) => Ok(commit),
            _ => Err(GitError::not_a_ccommit("think twice before do it!")),
        }
    }
}
