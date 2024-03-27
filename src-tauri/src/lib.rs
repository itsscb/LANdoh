pub mod app;
pub mod client;
mod model;
pub mod multicast;
mod pb;
mod server;
pub mod source;

pub fn shorten_path(name: String, path: String) -> String {
    let start: usize;
    match path.find(&name) {
        Some(n) => start = n,
        None => return String::from(""),
    };

    String::from(&path[start..path.len()])
}

#[test]
fn test_shorten_path() {
    let path = String::from("/home/user/Downloads/test/1/2/3");
    let result = shorten_path("test".to_string(), path);

    assert_eq!(result, "test/1/2/3".to_string());

    let path = String::from("C:\\users\\user1\\Downloads\\test\\1\\2\\3");
    let result = shorten_path("test".to_string(), path);

    assert_eq!(result, "test\\1\\2\\3".to_string());
}
