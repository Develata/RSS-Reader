#[derive(Debug, Clone)]
pub struct WebDavConfigSync {
    pub endpoint: String,
    pub remote_path: String,
}

impl WebDavConfigSync {
    pub fn new(endpoint: impl Into<String>, remote_path: impl Into<String>) -> Self {
        Self { endpoint: endpoint.into(), remote_path: remote_path.into() }
    }
}
