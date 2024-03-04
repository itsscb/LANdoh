use sha3::{Digest, Sha3_256};
use std::{error::Error, fs::File, io, path::PathBuf};

pub enum OSPATHS {
    APPDATA,
    PROGRAMDATA,
}

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

pub fn file_hash(path: PathBuf) -> Result<String, Box<dyn Error>> {
    let mut hasher = Sha3_256::new();
    let mut file = File::open(&path)?;

    let _ = io::copy(&mut file, &mut hasher)?;
    let hash = format!("{:X}", hasher.finalize());

    println!("{:?}\t{:?}", path, hash);

    Ok(hash)
}

#[test]
fn test_file_hash() {
    let hash = file_hash(PathBuf::from("testdir/file_root")).expect("could not generate hash");
    assert_eq!(
        hash,
        "7C7DDAFABDB6D48A84CFCD7EA6AD2FDD7EA73C1E21A2AFCCE8E625B10E0D9A0C".to_string()
    );
}
