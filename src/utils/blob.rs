use std::{
    io::{
        self,
        Write,
    },
    result,
    fmt,
    convert::TryFrom,
    error::Error,
    path::PathBuf
};

use crate::utils::{
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


#[derive(Clone)]
pub struct Blob(pub Vec<u8>);
impl ObjType for Blob {
    const VALUE: &'static str = "blob";
    const MODE: u32 = 0o100644;
}

impl TryFrom<Vec<u8>> for Blob {
    type Error = Box<dyn Error>;

    fn try_from(bytes: Vec<u8>) -> result::Result<Self, Self::Error> {
        let _ = parse_meta(&bytes)
            .map_err(|x|x.to_string())
            .map_err(GitError::invalid_obj)?;
        let index = bytes.iter().position(|&b| b == b'\0').expect("assert parse_meta implementation fails");
        Ok(Blob(bytes[index + 1..].to_vec()))
    }
}

impl From<Blob> for Vec<u8> {
    fn from(val: Blob) -> Vec<u8> {
        val.0
    }
}

impl fmt::Display for Blob {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes: Vec<u8> = self.clone().into();
        let _ = io::stdout().write_all(&bytes);
        Ok(())
    }
}

impl TryFrom<Obj> for Blob {
    type Error = Box<dyn Error>;

    fn try_from(obj: Obj) -> Result<Blob> {
        match obj {
            Obj::B(blob) => Ok(blob),
            _ => Err(GitError::not_a_bblob("think twice before do it!")),
        }
    }
}
