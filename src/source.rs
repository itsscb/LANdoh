#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct Source {
    pub id: String,
    pub nickname: String,
    pub shared_directories: Vec<String>,
    pub ip: Option<String>,
}

impl Source {
    pub fn new(
        id: String,
        nickname: String,
        ip: Option<String>,
        shared_directories: Vec<String>,
    ) -> Self {
        Source {
            id,
            nickname,
            ip,
            shared_directories,
        }
    }
}
