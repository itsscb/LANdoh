use std::str;

#[allow(dead_code, unused_imports)]
use self::pb_proto::Directory;

mod pb_proto {
    include!("pb.rs");
}

#[allow(dead_code)]
pub enum OSPATHS {
    APPDATA,
    PROGRAMDATA,
}

#[allow(dead_code)]
pub const CHUNK_SIZE: usize = 4069;

#[allow(dead_code)]
pub fn contains_partial_path(path: Option<&str>, paths: &Vec<String>) -> bool {
    match path {
        None => return false,
        Some(path) => {
            for p in paths {
                if path.starts_with(p) {
                    return true;
                }
            }
        }
    };
    false
}

#[test]
fn test_contains_partial_path() {
    let paths = vec!["testdir/sub/a", "testdir/sub/b", "testdir\\sub\\c"];
    let dir = Directory {
        name: "test".to_string(),
        paths: paths.clone().into_iter().map(String::from).collect(),
    };

    assert!(!contains_partial_path(None, &dir.paths));
    assert!(!contains_partial_path(Some("/etc/passwd"), &dir.paths));

    assert!(contains_partial_path(
        Some("testdir\\sub\\c\\blub"),
        &dir.paths
    ));
}
