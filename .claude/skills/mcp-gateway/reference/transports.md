## Transport Implementations

### Transport Trait

```rust
#[async_trait]
pub trait McpTransport: Send + Sync {
    /// Send a request and wait for response
    async fn send_request(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse, McpError>;

    /// Send a notification (no response expected)
    async fn send_notification(&self, notification: JsonRpcRequest) -> Result<(), McpError>;

    /// Check if transport is connected
    fn is_connected(&self) -> bool;

    /// Close the transport
    async fn close(&self) -> Result<(), McpError>;
}
```

### HTTP Transport

```rust
pub struct HttpTransport {
    client: reqwest::Client,
    base_url: String,
    auth: Option<AuthConfig>,
}

impl HttpTransport {
    pub fn new(base_url: &str, auth: Option<AuthConfig>) -> Self {
        let mut client_builder = reqwest::Client::builder()
            .timeout(Duration::from_secs(30));

        Self {
            client: client_builder.build().unwrap(),
            base_url: base_url.to_string(),
            auth,
        }
    }
}

#[async_trait]
impl McpTransport for HttpTransport {
    async fn send_request(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse, McpError> {
        let mut req = self.client
            .post(&self.base_url)
            .header("Content-Type", "application/json");

        // Add authentication
        if let Some(auth) = &self.auth {
            req = match auth {
                AuthConfig::Bearer { token } => {
                    req.header("Authorization", format!("Bearer {}", token))
                }
                AuthConfig::ApiKey { header, key } => {
                    req.header(header, key)
                }
                AuthConfig::Basic { username, password } => {
                    req.basic_auth(username, Some(password))
                }
            };
        }

        let response = req
            .json(&request)
            .send()
            .await
            .map_err(|e| McpError::Transport(e.to_string()))?;

        if !response.status().is_success() {
            return Err(McpError::Transport(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        response
            .json()
            .await
            .map_err(|e| McpError::Transport(e.to_string()))
    }

    async fn send_notification(&self, notification: JsonRpcRequest) -> Result<(), McpError> {
        let _ = self.send_request(notification).await?;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        true  // HTTP is stateless
    }

    async fn close(&self) -> Result<(), McpError> {
        Ok(())  // Nothing to close for HTTP
    }
}
```

### SSE Transport

```rust
use futures::StreamExt;
use tokio::sync::mpsc;

pub struct SseTransport {
    base_url: String,
    event_sender: mpsc::Sender<JsonRpcRequest>,
    response_receiver: mpsc::Receiver<JsonRpcResponse>,
    connected: Arc<AtomicBool>,
}

impl SseTransport {
    pub async fn connect(base_url: &str, auth: Option<AuthConfig>) -> Result<Self, McpError> {
        let (event_tx, event_rx) = mpsc::channel(100);
        let (response_tx, response_rx) = mpsc::channel(100);
        let connected = Arc::new(AtomicBool::new(false));
        let connected_clone = connected.clone();

        // Start SSE listener
        let url = format!("{}/events", base_url);
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let response = client.get(&url).send().await;

            if let Ok(response) = response {
                connected_clone.store(true, Ordering::SeqCst);

                let mut stream = response.bytes_stream();
                let mut buffer = String::new();

                while let Some(chunk) = stream.next().await {
                    if let Ok(bytes) = chunk {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));

                        // Parse SSE events
                        while let Some(event_end) = buffer.find("\n\n") {
                            let event_data = buffer[..event_end].to_string();
                            buffer = buffer[event_end + 2..].to_string();

                            if let Some(data) = event_data.strip_prefix("data: ") {
                                if let Ok(response) = serde_json::from_str::<JsonRpcResponse>(data) {
                                    let _ = response_tx.send(response).await;
                                }
                            }
                        }
                    }
                }

                connected_clone.store(false, Ordering::SeqCst);
            }
        });

        Ok(Self {
            base_url: base_url.to_string(),
            event_sender: event_tx,
            response_receiver: response_rx,
            connected,
        })
    }
}

#[async_trait]
impl McpTransport for SseTransport {
    async fn send_request(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse, McpError> {
        // Send request via POST
        let client = reqwest::Client::new();
        client
            .post(&format!("{}/message", self.base_url))
            .json(&request)
            .send()
            .await
            .map_err(|e| McpError::Transport(e.to_string()))?;

        // Wait for response via SSE
        tokio::time::timeout(
            Duration::from_secs(30),
            self.response_receiver.recv()
        )
        .await
        .map_err(|_| McpError::Timeout)?
        .ok_or(McpError::Transport("Channel closed".to_string()))
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    async fn close(&self) -> Result<(), McpError> {
        self.connected.store(false, Ordering::SeqCst);
        Ok(())
    }
}
```

### WebSocket Transport

```rust
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub struct WebSocketTransport {
    sender: mpsc::Sender<Message>,
    response_receiver: mpsc::Receiver<JsonRpcResponse>,
    connected: Arc<AtomicBool>,
}

impl WebSocketTransport {
    pub async fn connect(url: &str) -> Result<Self, McpError> {
        let (ws_stream, _) = connect_async(url)
            .await
            .map_err(|e| McpError::Transport(e.to_string()))?;

        let (write, read) = ws_stream.split();
        let (tx, rx) = mpsc::channel(100);
        let (response_tx, response_rx) = mpsc::channel(100);
        let connected = Arc::new(AtomicBool::new(true));

        // Writer task
        let connected_writer = connected.clone();
        tokio::spawn(async move {
            let mut rx = rx;
            let mut write = write;
            while let Some(msg) = rx.recv().await {
                if write.send(msg).await.is_err() {
                    connected_writer.store(false, Ordering::SeqCst);
                    break;
                }
            }
        });

        // Reader task
        let connected_reader = connected.clone();
        tokio::spawn(async move {
            let mut read = read;
            while let Some(Ok(msg)) = read.next().await {
                if let Message::Text(text) = msg {
                    if let Ok(response) = serde_json::from_str::<JsonRpcResponse>(&text) {
                        let _ = response_tx.send(response).await;
                    }
                }
            }
            connected_reader.store(false, Ordering::SeqCst);
        });

        Ok(Self {
            sender: tx,
            response_receiver: response_rx,
            connected,
        })
    }
}

#[async_trait]
impl McpTransport for WebSocketTransport {
    async fn send_request(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse, McpError> {
        let json = serde_json::to_string(&request)
            .map_err(|e| McpError::Serialization(e.to_string()))?;

        self.sender
            .send(Message::Text(json))
            .await
            .map_err(|e| McpError::Transport(e.to_string()))?;

        tokio::time::timeout(
            Duration::from_secs(30),
            self.response_receiver.recv()
        )
        .await
        .map_err(|_| McpError::Timeout)?
        .ok_or(McpError::Transport("Channel closed".to_string()))
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    async fn close(&self) -> Result<(), McpError> {
        self.connected.store(false, Ordering::SeqCst);
        Ok(())
    }
}
```

### Stdio Transport

```rust
use tokio::process::{Child, Command};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub struct StdioTransport {
    child: Child,
    stdin: tokio::process::ChildStdin,
    response_receiver: mpsc::Receiver<JsonRpcResponse>,
}

impl StdioTransport {
    pub async fn spawn(command: &str, args: &[&str]) -> Result<Self, McpError> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| McpError::Transport(format!("Failed to spawn process: {}", e)))?;

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let (response_tx, response_rx) = mpsc::channel(100);

        // Reader task
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(response) = serde_json::from_str::<JsonRpcResponse>(&line) {
                    let _ = response_tx.send(response).await;
                }
            }
        });

        Ok(Self {
            child,
            stdin,
            response_receiver: response_rx,
        })
    }
}

#[async_trait]
impl McpTransport for StdioTransport {
    async fn send_request(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse, McpError> {
        let json = serde_json::to_string(&request)
            .map_err(|e| McpError::Serialization(e.to_string()))?;

        self.stdin
            .write_all(format!("{}\n", json).as_bytes())
            .await
            .map_err(|e| McpError::Transport(e.to_string()))?;

        self.stdin
            .flush()
            .await
            .map_err(|e| McpError::Transport(e.to_string()))?;

        tokio::time::timeout(
            Duration::from_secs(30),
            self.response_receiver.recv()
        )
        .await
        .map_err(|_| McpError::Timeout)?
        .ok_or(McpError::Transport("Channel closed".to_string()))
    }

    fn is_connected(&self) -> bool {
        // Check if process is still running
        true  // Would need to track process state
    }

    async fn close(&self) -> Result<(), McpError> {
        self.child.kill().await.ok();
        Ok(())
    }
}
```

---
