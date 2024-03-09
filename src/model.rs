use std::str;

#[allow(dead_code)]
pub enum OSPATHS {
    APPDATA,
    PROGRAMDATA,
}

#[allow(dead_code)]
pub const CHUNK_SIZE: usize = 4069;

#[derive(Debug)]
pub struct Directory {
    pub name: String,
    pub paths: Vec<String>,
}

impl Directory {
    #[allow(dead_code)]
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
