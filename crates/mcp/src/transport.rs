use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Transport {
    Stdio {
        command: String,
        args: Vec<String>,
        env: std::collections::HashMap<String, String>,
    },
    Sse {
        url: String,
        headers: std::collections::HashMap<String, String>,
    },
    StreamableHttp {
        url: String,
        headers: std::collections::HashMap<String, String>,
    },
    WebSocket {
        url: String,
        headers: std::collections::HashMap<String, String>,
    },
}
