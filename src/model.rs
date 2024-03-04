use md5::Md5;
use sha3::Digest;
use std::{
    error::Error,
    fs::File,
    io::{self, BufReader, Read, Seek, SeekFrom},
    path::PathBuf,
    str,
    sync::Mutex,
};

pub enum OSPATHS {
    APPDATA,
    PROGRAMDATA,
}

pub const CHUNK_SIZE: usize = 4069;

#[derive(Debug)]
pub struct Directory {
    pub name: String,
    pub paths: Vec<String>,
}

impl Directory {
    pub fn contains_partial_path(&self, path: Option<&str>) -> bool {
        match path {
            None => return false,
            Some(path) => {
                for p in &self.paths {
                    if path.starts_with(p) {
                        return true;
                    }
                }
            }
        };
        false
    }
}

impl PartialEq for Directory {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
    fn ne(&self, other: &Self) -> bool {
        self.name != other.name
    }
}

#[test]
fn test_contains_partial_path() {
    let paths = vec!["testdir/sub/a", "testdir/sub/b", "testdir\\sub\\c"];
    let dir = Directory {
        name: "test".to_string(),
        paths: paths.clone().into_iter().map(String::from).collect(),
    };

    assert!(!dir.contains_partial_path(None));
    assert!(!dir.contains_partial_path(Some("/etc/passwd")));

    assert!(dir.contains_partial_path(Some("testdir\\sub\\c\\blub")));
}

pub fn file_hash(path: &PathBuf) -> Result<String, Box<dyn Error>> {
    let mut hasher = Md5::new();
    let mut file = File::open(&path)?;

    let _ = io::copy(&mut file, &mut hasher)?;
    let hash = format!("{:X}", hasher.finalize());

    Ok(hash)
}

#[test]
fn test_file_hash() {
    let hash = file_hash(&PathBuf::from("testdir/file_root")).expect("could not generate hash");
    assert_eq!(hash, "A8E590E9AEC56854C40856A2A7742B81".to_string());
}

pub struct FileHasher {
    hasher: Mutex<Md5>,
}

impl FileHasher {
    pub fn new() -> FileHasher {
        FileHasher {
            hasher: Mutex::new(Md5::new()),
        }
    }

    pub fn update(&self, data: Vec<u8>) {
        self.hasher.lock().unwrap().update(data);
    }

    pub fn finalize(self) -> String {
        format!("{:X}", self.hasher.lock().unwrap().clone().finalize())
    }
}

#[test]
fn test_file_hasher() {
    let hasher = FileHasher::new();

    let path = PathBuf::from("testdir/file_root");
    let size = path.metadata().expect("failed to read file size").len();

    let mut read = 0;
    let mut file = File::open(&path).expect("failed to read file");

    loop {
        let chunk: usize;
        if size < read {
            break;
        }
        if size - read >= CHUNK_SIZE as u64 {
            chunk = CHUNK_SIZE;
        } else {
            chunk = (size - read) as usize;
        }
        let mut buf = vec![0; chunk];

        file.seek(SeekFrom::Start(read))
            .expect("failed to set byte position on read");
        let mut reader = BufReader::new(&file);
        reader
            .read_exact(&mut buf)
            .expect("failed to read chunk into buffer");

        hasher.update(buf);

        read += CHUNK_SIZE as u64;
    }

    let hash = hasher.finalize();
    assert_eq!(hash, "A8E590E9AEC56854C40856A2A7742B81".to_string());
}
