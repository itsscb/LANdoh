use chrono::{DateTime, Utc};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct Source {
    pub id: String,
    pub nickname: String,
    pub shared_directories: Vec<String>,
    pub ip: Option<String>,
    pub timestamp: DateTime<Utc>,
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
            timestamp: Utc::now(),
        }
    }

    pub fn update(
        &mut self,
        nickname: String,
        ip: Option<String>,
        shared_directories: Vec<String>,
    ) {
        self.nickname = nickname;
        self.ip = ip;
        self.shared_directories = shared_directories;
        self.timestamp = Utc::now();
    }

    pub fn is_outdated(&self) -> bool {
        let diff = Utc::now().time() - self.timestamp.time();
        diff.num_seconds() > 30
    }
}
