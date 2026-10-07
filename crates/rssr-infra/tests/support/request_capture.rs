//! Loopback-only HTTP fixture, shared with the configuration roundtrip test.
use std::sync::{Arc, Mutex};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};
use url::Url;

#[derive(Clone, Debug)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

pub struct Response {
    status: u16,
    headers: String,
    body: Vec<u8>,
}

impl Response {
    pub fn new(status: u16, headers: &str, body: impl Into<Vec<u8>>) -> Self {
        Self { status, headers: headers.into(), body: body.into() }
    }
}

pub struct Server {
    pub url: Url,
    requests: Arc<Mutex<Vec<Request>>>,
    task: JoinHandle<()>,
}

impl Server {
    pub async fn start(handler: impl Fn(&Request) -> Response + Send + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind loopback fixture");
        let url = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let task = tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.expect("accept request");
                let mut raw = Vec::new();
                let mut buf = [0_u8; 4096];
                let header_end = loop {
                    let read = stream.read(&mut buf).await.expect("read request headers");
                    assert!(read > 0, "request ended before headers");
                    raw.extend_from_slice(&buf[..read]);
                    if let Some(idx) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                        break idx + 4;
                    }
                    assert!(raw.len() < 64 * 1024, "fixture header limit");
                };
                let head = String::from_utf8_lossy(&raw[..header_end]);
                let mut lines = head.lines();
                let mut parts = lines.next().unwrap().split_whitespace();
                let mut request = Request {
                    method: parts.next().unwrap().into(),
                    path: parts.next().unwrap().into(),
                    headers: lines
                        .filter_map(|line| line.split_once(':'))
                        .map(|(name, value)| (name.into(), value.trim().into()))
                        .collect(),
                    body: Vec::new(),
                };
                let content_length = request
                    .header("content-length")
                    .map(|v| v.parse::<usize>().unwrap())
                    .unwrap_or(0);
                assert!(content_length <= 4 * 1024 * 1024, "fixture body limit");
                while raw.len() < header_end + content_length {
                    let read = stream.read(&mut buf).await.expect("read request body");
                    assert!(read > 0, "request ended before body");
                    raw.extend_from_slice(&buf[..read]);
                }
                request.body.extend_from_slice(&raw[header_end..header_end + content_length]);
                let response = handler(&request);
                captured.lock().unwrap().push(request);
                let head = format!(
                    "HTTP/1.1 {} Fixture\r\nConnection: close\r\nContent-Length: {}\r\n{}\r\n",
                    response.status,
                    response.body.len(),
                    response.headers,
                );
                // Size limits may close the client connection early.
                let _ = stream.write_all(head.as_bytes()).await;
                let _ = stream.write_all(&response.body).await;
                let _ = stream.shutdown().await;
            }
        });
        Self { url, requests, task }
    }

    pub fn requests(&self) -> Vec<Request> {
        assert!(!self.task.is_finished(), "fixture stopped unexpectedly");
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
