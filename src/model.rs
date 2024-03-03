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

pub enum OSPATHS {
    APPDATA,
    PROGRAMDATA,
}
