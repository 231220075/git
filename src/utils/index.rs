use std::path::{PathBuf,Path};
use std::fs::{File, OpenOptions};
use std::io::{Write, BufWriter, Read, BufReader, BufRead};
use byteorder::{ReadBytesExt, BigEndian};
use sha1::{Sha1, Digest};
use nom::{
    bytes::complete::{tag, take, take_until},
    number::complete::be_u32,
    IResult,
};
use std::iter::repeat_n;
use crate::{
    GitError,
    Result,
};

#[derive(Debug)]
pub struct IndexEntry {
    pub mode: u32,
    pub hash: String,
    pub name: String,
}

impl IndexEntry {

    pub fn new(mode: u32, hash: String, name: String) -> Self {
        match mode {
            0o100644 | 0o100755 | 0o120000 | 0o040000 => (),
            _ => panic!("Invalid file mode: {:o}", mode),
        }
        IndexEntry { mode, hash, name }
    }

}
#[derive(Debug)]
pub struct Index {
    pub entries: Vec<IndexEntry>,
}

impl Default for Index {
    fn default() -> Self {
        Self::new()
    }
}

impl Index {
    pub fn new() -> Self {
        Index { entries: Vec::new() }
    }

    pub fn add_entry(&mut self, new_entry: IndexEntry) {
        self.entries.push(new_entry);
    }

    pub fn write_to_file(&self, path: &Path) -> std::io::Result<()> {
        use sha1::{Sha1, Digest};
        use std::io::Seek;
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        let mut writer = BufWriter::new(file);
        let mut buffer = Vec::new();

        // writer.write_all(b"DIRC")?;
        // writer.write_all(&2u32.to_be_bytes())?;
        // writer.write_all(&(self.entries.len() as u32).to_be_bytes())?;//header = signature + version + number of entries

        // for entry in &self.entries {
        //     writer.write_all(&entry.mode.to_be_bytes())?; 

        //     let hash_bytes = hex::decode(&entry.hash).map_err(|_| {
        //         std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid hash format")
        //     })?;
        //     writer.write_all(&hash_bytes)?; 

        //     writer.write_all(entry.name.as_bytes())?; 
        //     writer.write_all(&[0])?; 
        // }
        // Ok(())
        buffer.extend_from_slice(b"DIRC");
        buffer.extend_from_slice(&2u32.to_be_bytes());
        buffer.extend_from_slice(&(self.entries.len() as u32).to_be_bytes());

        for entry in &self.entries {
            // println!("write {} to file {}", entry.name, path.display());
            buffer.extend_from_slice(&0u32.to_be_bytes()); // ctime
            buffer.extend_from_slice(&0u32.to_be_bytes()); // ctime_nsec
            buffer.extend_from_slice(&0u32.to_be_bytes()); // mtime
            buffer.extend_from_slice(&0u32.to_be_bytes()); // mtime_nsec
            buffer.extend_from_slice(&0u32.to_be_bytes()); // dev
            buffer.extend_from_slice(&0u32.to_be_bytes()); // ino
            buffer.extend_from_slice(&entry.mode.to_be_bytes());
            buffer.extend_from_slice(&0u32.to_be_bytes()); // uid
            buffer.extend_from_slice(&0u32.to_be_bytes()); // gid
            buffer.extend_from_slice(&0u32.to_be_bytes()); // file size

            let hash_bytes = hex::decode(&entry.hash).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid hash format")
            })?;
            buffer.extend_from_slice(&hash_bytes);
            let name_bytes = entry.name.as_bytes();
            let name_len = name_bytes.len();
            let stage: u16 = 0;
            let flags: u16 = ((stage & 0x3) << 12) | ((name_len as u16) & 0x0FFF);
            buffer.extend_from_slice(&flags.to_be_bytes());
            buffer.extend_from_slice(entry.name.as_bytes());
            buffer.push(0);

        // 计算对齐
        let entry_len = 63 + entry.name.len(); // 62字节固定+name
        let pad = (8 - (entry_len % 8)) % 8;
        buffer.extend(std::iter::repeat_n(0, pad));
    }
    let mut hasher = Sha1::new();
    hasher.update(&buffer);
    let checksum = hasher.finalize();
    buffer.extend_from_slice(&checksum);

        writer.write_all(&buffer)?;
        writer.flush()?;
        Ok(())
    }

    // pub fn read_from_file(&self, path: &Path) -> std::io::Result<Self> {
    //     let file = File::open(path)?;
    //     let mut reader = BufReader::new(file);
    //     let mut index = Index::new();

    //     let mut signature = [0u8; 4];
    //     reader.read_exact(&mut signature)?;
    //     if &signature != b"DIRC" {
    //         return Err(std::io::Error::new(
    //             std::io::ErrorKind::InvalidData,
    //             "Invalid index file signature",
    //         ));
    //     }
    // let version = reader.read_u32::<BigEndian>()?;
    // if version != 2 {
    //     return Err(std::io::Error::new(
    //         std::io::ErrorKind::InvalidData,
    //         "Unsupported index file version",
    //     ));
    // }
    // let num_entries = reader.read_u32::<BigEndian>()?;

    // for _ in 0..num_entries {
    //     let mode = reader.read_u32::<BigEndian>()?;
    //     let mut hash = [0u8; 20];
    //     reader.read_exact(&mut hash)?;

    //     let mut name = Vec::new();
    //     reader.read_until(0, &mut name)?;
    //     if name.is_empty() || name.last() != Some(&0) {
    //         return Err(std::io::Error::new(
    //             std::io::ErrorKind::InvalidData,
    //             "Invalid name format in index file",
    //             ));
    //         }
    //         name.pop(); 

    //         let name_str = String::from_utf8(name).map_err(|_| {
    //             std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid UTF-8 in name")
    //         })?;

    //         index.add_entry(IndexEntry::new(
    //             mode,
    //             hex::encode(hash),
    //             name_str,
    //         ));
    //     }
    //     Ok(index)
    // }
    fn parse_index(input: &[u8]) -> IResult<&[u8], Index> {
        let (input, _) = tag("DIRC")(input)?;
        let (input, _version) = be_u32(input)?;
        let (input, entry_count) = be_u32(input)?;

        let mut entries = Vec::new();
        let mut input = input;
        for _ in 0..entry_count {
            let (rest, entry) = Self::parse_entry(input)?;
            entries.push(entry);
            input = rest;
        }
        // 跳过校验和
        let (_input, _checksum) = take(20usize)(input)?;
        Ok((_input, Index { entries }))
    }

    fn parse_entry(input: &[u8]) -> IResult<&[u8], IndexEntry> {
        let (input, _ctime) = take(4usize)(input)?;
        let (input, _ctime_nsec) = take(4usize)(input)?;
        let (input, _mtime) = take(4usize)(input)?;
        let (input, _mtime_nsec) = take(4usize)(input)?;
        let (input, _dev) = take(4usize)(input)?;
        let (input, _ino) = take(4usize)(input)?;
        let (input, mode_bytes) = take(4usize)(input)?;
        let mode = u32::from_be_bytes(mode_bytes.try_into().unwrap());
        let (input, _uid) = take(4usize)(input)?;
        let (input, _gid) = take(4usize)(input)?;
        let (input, _size) = take(4usize)(input)?;
        let (input, hash) = take(20usize)(input)?;
        let (input, _flags) = take(2usize)(input)?;

        // 文件名直到0字节
        let nul_pos = input.iter().position(|&b| b == 0).unwrap();
        let name = &input[..nul_pos];
        let input = &input[nul_pos + 1..];

        // 对齐到8字节
        let entry_len = 63 + name.len();
        let pad = (8 - (entry_len % 8)) % 8;
        let input = &input[pad..];

        Ok((input, IndexEntry::new(
                    mode,
                    hex::encode(hash),
                    String::from_utf8(name.to_vec()).unwrap(),
        )))
    }


    pub fn read_from_file(&self, path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path)?;
        let (_, index) = Self::parse_index(&bytes).map_err(|_| {
            GitError::InvalidCommand(path.to_str().unwrap().to_string())
        })?;
        Ok(index)
    }



    pub fn remove_entry(&mut self, name: &str) -> bool {
        let original_len = self.entries.len();
        self.entries.retain(|entry| entry.name != name);
        original_len != self.entries.len()
    }
}
