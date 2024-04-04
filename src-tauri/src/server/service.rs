use core::fmt;

#[allow(dead_code)]
#[derive(Debug)]
pub enum Service {
    Healthz,
    GetDirectory,
    GetFile,
    ListDirectories,
}

impl fmt::Display for Service {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
