//! DevEco IDE hot-reload WebSocket notification server.
//!
//! Implements a best-effort version of the DevEco IDE hot-reload protocol
//! (section 3.9 of the gap analysis).  The exact proprietary protocol is
//! unknown; this module implements the HTTP WebSocket upgrade handshake
//! (RFC 6455) and sends JSON build-status messages in the format that DevEco
//! is believed to consume based on reverse-engineering common IDE protocols.
//!
//! ## Port
//!
//! The server listens on port 61001, the known DevEco hot-reload port.
//!
//! ## Message format
//!
//! ```json
//! {"type":"hotReload","event":"BUILD_COMPLETE","module":"entry","status":"success"}
//! ```
//!
//! ## Usage
//!
//! ```no_run
//! use arkdown_watcher::deveco_ws::{DevEcoWsServer, DevEcoEvent};
//!
//! #[tokio::main]
//! async fn main() {
//!     let server = DevEcoWsServer::new(61001);
//!     let handle = server.start().await.expect("server start");
//!
//!     // Broadcast a build-complete event to all connected IDE clients.
//!     handle.broadcast(DevEcoEvent::build_complete("entry", true)).await;
//! }
//! ```
//!
//! ## Protocol notes
//!
//! The WebSocket handshake is implemented from scratch using only the standard
//! library and `tokio` so that no additional crate dependency is required.
//! The implementation covers:
//!
//! - HTTP/1.1 `GET` upgrade request parsing.
//! - `Sec-WebSocket-Accept` header computation (SHA-1 of key + GUID, base64).
//! - WebSocket text frame encoding (RFC 6455 §5.2).
//! - Graceful connection close when clients disconnect.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;

/// Default DevEco hot-reload port.
pub const DEVECO_WS_PORT: u16 = 61001;

/// A build-status event to broadcast to connected DevEco IDE clients.
#[derive(Debug, Clone)]
pub struct DevEcoEvent {
    /// JSON payload to send as a WebSocket text frame.
    pub payload: String,
}

impl DevEcoEvent {
    /// Create a BUILD_COMPLETE event.
    pub fn build_complete(module: &str, success: bool) -> Self {
        let status = if success { "success" } else { "failure" };
        DevEcoEvent {
            payload: format!(
                r#"{{"type":"hotReload","event":"BUILD_COMPLETE","module":"{}","status":"{}"}}"#,
                escape_json(module),
                status
            ),
        }
    }

    /// Create a BUILD_START event.
    pub fn build_start(module: &str) -> Self {
        DevEcoEvent {
            payload: format!(
                r#"{{"type":"hotReload","event":"BUILD_START","module":"{}","status":"running"}}"#,
                escape_json(module)
            ),
        }
    }

    /// Create a BUILD_ERROR event with a message.
    pub fn build_error(module: &str, message: &str) -> Self {
        DevEcoEvent {
            payload: format!(
                r#"{{"type":"hotReload","event":"BUILD_ERROR","module":"{}","message":"{}","status":"failure"}}"#,
                escape_json(module),
                escape_json(message)
            ),
        }
    }

    /// Create a custom event with an arbitrary JSON payload.
    pub fn custom(payload: String) -> Self {
        DevEcoEvent { payload }
    }
}

/// Handle to a running [`DevEcoWsServer`] that can broadcast events.
#[derive(Clone)]
pub struct DevEcoServerHandle {
    tx: broadcast::Sender<DevEcoEvent>,
    /// Bound address (useful for tests that use port 0).
    pub addr: SocketAddr,
}

impl DevEcoServerHandle {
    /// Broadcast `event` to all currently connected WebSocket clients.
    ///
    /// Returns the number of clients that received the event (0 when no
    /// clients are connected).
    pub async fn broadcast(&self, event: DevEcoEvent) -> usize {
        match self.tx.send(event) {
            Ok(n) => n,
            Err(_) => 0, // No receivers — no clients connected.
        }
    }
}

/// DevEco IDE hot-reload WebSocket server.
///
/// Accepts WebSocket connections from DevEco IDE on the configured port and
/// broadcasts build-status events to all connected clients.
pub struct DevEcoWsServer {
    port: u16,
}

impl DevEcoWsServer {
    /// Create a new server that will listen on `port`.
    pub fn new(port: u16) -> Self {
        DevEcoWsServer { port }
    }

    /// Start the server and return a [`DevEcoServerHandle`] for broadcasting.
    ///
    /// The server runs in a background `tokio` task.  Drop the handle to stop
    /// accepting new connections (existing connections are closed when the
    /// broadcast channel is dropped).
    pub async fn start(self) -> Result<DevEcoServerHandle, std::io::Error> {
        let addr: SocketAddr = format!("127.0.0.1:{}", self.port).parse().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("{}", e))
        })?;
        let listener = TcpListener::bind(addr).await?;
        let bound_addr = listener.local_addr()?;

        // Broadcast channel: up to 64 buffered events per subscriber.
        let (tx, _rx) = broadcast::channel::<DevEcoEvent>(64);
        let tx = Arc::new(tx);

        let tx_clone = tx.clone();
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, peer)) => {
                        tracing::debug!(
                            peer = %peer,
                            "deveco-ws: new connection from DevEco IDE"
                        );
                        let rx = tx_clone.subscribe();
                        tokio::spawn(handle_connection(stream, rx));
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "deveco-ws: accept error");
                        break;
                    }
                }
            }
        });

        Ok(DevEcoServerHandle {
            tx: (*tx).clone(),
            addr: bound_addr,
        })
    }
}

// ---------------------------------------------------------------------------
// Connection handler
// ---------------------------------------------------------------------------

/// Handle a single WebSocket connection from DevEco IDE.
async fn handle_connection(
    mut stream: TcpStream,
    mut rx: broadcast::Receiver<DevEcoEvent>,
) {
    // Step 1: Read the HTTP upgrade request.
    let ws_key = match read_http_upgrade(&mut stream).await {
        Ok(key) => key,
        Err(e) => {
            tracing::debug!(error = %e, "deveco-ws: HTTP upgrade failed");
            return;
        }
    };

    // Step 2: Send the HTTP 101 Switching Protocols response.
    if let Err(e) = send_http_upgrade_response(&mut stream, &ws_key).await {
        tracing::debug!(error = %e, "deveco-ws: failed to send upgrade response");
        return;
    }

    tracing::debug!("deveco-ws: WebSocket handshake complete");

    // Step 3: Forward broadcast events as WebSocket text frames.
    loop {
        match rx.recv().await {
            Ok(event) => {
                if let Err(e) = send_ws_text_frame(&mut stream, &event.payload).await {
                    tracing::debug!(error = %e, "deveco-ws: client disconnected");
                    return;
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!(skipped = n, "deveco-ws: client lagged, skipping events");
            }
            Err(broadcast::error::RecvError::Closed) => {
                // Server is shutting down.
                return;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// HTTP WebSocket upgrade handshake (RFC 6455)
// ---------------------------------------------------------------------------

/// Read an HTTP/1.1 WebSocket upgrade request and return the
/// `Sec-WebSocket-Key` header value.
///
/// Reads up to 4096 bytes (sufficient for any reasonable HTTP upgrade
/// request) and extracts the key header.
async fn read_http_upgrade(stream: &mut TcpStream) -> Result<String, String> {
    let mut buf = vec![0u8; 4096];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| format!("read: {}", e))?;
    if n == 0 {
        return Err("connection closed before upgrade".to_string());
    }

    let request = std::str::from_utf8(&buf[..n]).map_err(|e| format!("utf8: {}", e))?;

    // Extract Sec-WebSocket-Key header (case-insensitive search).
    for line in request.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("sec-websocket-key:") {
            let key = line
                .splitn(2, ':')
                .nth(1)
                .unwrap_or("")
                .trim()
                .to_string();
            if !key.is_empty() {
                return Ok(key);
            }
        }
    }

    Err("Sec-WebSocket-Key header not found".to_string())
}

/// Send the HTTP 101 Switching Protocols response.
///
/// Computes the `Sec-WebSocket-Accept` header value using the standard
/// RFC 6455 algorithm:
/// 1. Concatenate the client key with the GUID `"258EAFA5-E914-47DA-95CA-C5AB0DC85B11"`.
/// 2. SHA-1 hash the result.
/// 3. Base64-encode the hash.
async fn send_http_upgrade_response(
    stream: &mut TcpStream,
    client_key: &str,
) -> Result<(), std::io::Error> {
    let accept = compute_ws_accept(client_key);
    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {}\r\n\
         \r\n",
        accept
    );
    stream.write_all(response.as_bytes()).await
}

/// Compute the `Sec-WebSocket-Accept` value for the given client key.
///
/// RFC 6455 §1.3: `base64(SHA-1(key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"))`.
fn compute_ws_accept(client_key: &str) -> String {
    const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    let input = format!("{}{}", client_key, WS_GUID);
    let hash = sha1_bytes(input.as_bytes());
    base64_encode(&hash)
}

// ---------------------------------------------------------------------------
// WebSocket frame encoding (RFC 6455 §5.2)
// ---------------------------------------------------------------------------

/// Send a WebSocket text frame containing `payload`.
///
/// Encodes a minimal unmasked text frame (server-to-client frames are not
/// masked per RFC 6455 §5.1).
async fn send_ws_text_frame(stream: &mut TcpStream, payload: &str) -> Result<(), std::io::Error> {
    let frame = encode_ws_text_frame(payload);
    stream.write_all(&frame).await
}

/// Encode `payload` as a WebSocket text frame (RFC 6455 §5.2).
///
/// Frame layout:
/// - Byte 0: `0x81` (FIN=1, opcode=1 text).
/// - Byte 1: payload length (7-bit for ≤125 bytes, 126 + 2-byte big-endian for ≤65535).
/// - Payload bytes.
///
/// Server frames are unmasked (MASK bit = 0).
fn encode_ws_text_frame(payload: &str) -> Vec<u8> {
    let data = payload.as_bytes();
    let len = data.len();

    let mut frame = Vec::with_capacity(len + 10);
    // FIN=1, RSV=0, opcode=0x1 (text)
    frame.push(0x81);

    if len <= 125 {
        frame.push(len as u8);
    } else if len <= 65535 {
        frame.push(126);
        frame.push((len >> 8) as u8);
        frame.push((len & 0xFF) as u8);
    } else {
        // 64-bit length (very large payloads — unlikely in practice).
        frame.push(127);
        for i in (0..8).rev() {
            frame.push(((len >> (i * 8)) & 0xFF) as u8);
        }
    }

    frame.extend_from_slice(data);
    frame
}

// ---------------------------------------------------------------------------
// Cryptographic helpers (SHA-1 + base64 for WebSocket handshake)
// ---------------------------------------------------------------------------

/// Minimal SHA-1 implementation for the WebSocket handshake.
///
/// SHA-1 is only used for the RFC 6455 handshake key derivation — it is NOT
/// used for any security-sensitive operation.  Using a minimal inline
/// implementation avoids adding a new crate dependency.
fn sha1_bytes(input: &[u8]) -> [u8; 20] {
    // SHA-1 constants (FIPS 180-4).
    let mut h: [u32; 5] = [
        0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0,
    ];

    // Pre-processing: padding.
    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut msg: Vec<u8> = input.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0x00);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    // Process each 512-bit (64-byte) block.
    for block in msg.chunks_exact(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([block[i * 4], block[i * 4 + 1], block[i * 4 + 2], block[i * 4 + 3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);

        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1u32),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDCu32),
                _ => (b ^ c ^ d, 0xCA62C1D6u32),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    // Produce the 20-byte digest.
    let mut out = [0u8; 20];
    for (i, &val) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&val.to_be_bytes());
    }
    out
}

/// Minimal base64 encoder for the WebSocket handshake.
///
/// Encodes `input` using the standard Base64 alphabet (RFC 4648 §4).
fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::new();
    let mut i = 0;
    while i < input.len() {
        let b0 = input[i] as u32;
        let b1 = input.get(i + 1).copied().unwrap_or(0) as u32;
        let b2 = input.get(i + 2).copied().unwrap_or(0) as u32;

        out.push(ALPHABET[((b0 >> 2) & 0x3F) as usize] as char);
        out.push(ALPHABET[(((b0 << 4) | (b1 >> 4)) & 0x3F) as usize] as char);

        if i + 1 < input.len() {
            out.push(ALPHABET[(((b1 << 2) | (b2 >> 6)) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if i + 2 < input.len() {
            out.push(ALPHABET[(b2 & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }

        i += 3;
    }
    out
}

// ---------------------------------------------------------------------------
// JSON string escaping
// ---------------------------------------------------------------------------

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Event construction
    // -----------------------------------------------------------------------

    #[test]
    fn test_build_complete_event_success() {
        let ev = DevEcoEvent::build_complete("entry", true);
        assert!(ev.payload.contains("BUILD_COMPLETE"));
        assert!(ev.payload.contains("\"status\":\"success\""));
        assert!(ev.payload.contains("\"module\":\"entry\""));
    }

    #[test]
    fn test_build_complete_event_failure() {
        let ev = DevEcoEvent::build_complete("feature", false);
        assert!(ev.payload.contains("\"status\":\"failure\""));
        assert!(ev.payload.contains("\"module\":\"feature\""));
    }

    #[test]
    fn test_build_start_event() {
        let ev = DevEcoEvent::build_start("entry");
        assert!(ev.payload.contains("BUILD_START"));
        assert!(ev.payload.contains("\"status\":\"running\""));
    }

    #[test]
    fn test_build_error_event() {
        let ev = DevEcoEvent::build_error("entry", "compile failed");
        assert!(ev.payload.contains("BUILD_ERROR"));
        assert!(ev.payload.contains("compile failed"));
        assert!(ev.payload.contains("\"status\":\"failure\""));
    }

    #[test]
    fn test_custom_event() {
        let payload = r#"{"type":"custom","data":42}"#.to_string();
        let ev = DevEcoEvent::custom(payload.clone());
        assert_eq!(ev.payload, payload);
    }

    #[test]
    fn test_event_payload_is_valid_json() {
        let events = [
            DevEcoEvent::build_complete("entry", true),
            DevEcoEvent::build_start("entry"),
            DevEcoEvent::build_error("entry", "oops"),
        ];
        for ev in &events {
            let parsed: serde_json::Value =
                serde_json::from_str(&ev.payload).expect("event payload must be valid JSON");
            assert!(parsed.get("type").is_some(), "event must have 'type' field");
            assert!(parsed.get("event").is_some(), "event must have 'event' field");
        }
    }

    // -----------------------------------------------------------------------
    // SHA-1 and base64
    // -----------------------------------------------------------------------

    #[test]
    fn test_sha1_empty_string() {
        // SHA-1("") = da39a3ee5e6b4b0d3255bfef95601890afd80709
        let hash = sha1_bytes(b"");
        let hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
        assert_eq!(hex, "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    }

    #[test]
    fn test_sha1_abc() {
        // SHA-1("abc") = a9993e364706816aba3e25717850c26c9cd0d89d
        let hash = sha1_bytes(b"abc");
        let hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
        assert_eq!(hex, "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn test_base64_encode_known_vectors() {
        // RFC 4648 test vectors.
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn test_ws_accept_known_vector() {
        // RFC 6455 §1.3 example:
        // Client key: "dGhlIHNhbXBsZSBub25jZQ=="
        // Expected accept: "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        let accept = compute_ws_accept("dGhlIHNhbXBsZSBub25jZQ==");
        assert_eq!(accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    // -----------------------------------------------------------------------
    // WebSocket frame encoding
    // -----------------------------------------------------------------------

    #[test]
    fn test_ws_frame_small_payload() {
        let frame = encode_ws_text_frame("hello");
        // Byte 0: 0x81 (FIN + text opcode)
        assert_eq!(frame[0], 0x81);
        // Byte 1: payload length (5), unmasked
        assert_eq!(frame[1], 5);
        // Payload
        assert_eq!(&frame[2..], b"hello");
    }

    #[test]
    fn test_ws_frame_126_byte_payload() {
        let payload = "x".repeat(200);
        let frame = encode_ws_text_frame(&payload);
        assert_eq!(frame[0], 0x81);
        // Byte 1: 126 (indicates 2-byte extended length)
        assert_eq!(frame[1], 126);
        // Bytes 2-3: big-endian length
        let len = u16::from_be_bytes([frame[2], frame[3]]) as usize;
        assert_eq!(len, 200);
        assert_eq!(&frame[4..], payload.as_bytes());
    }

    #[test]
    fn test_ws_frame_empty_payload() {
        let frame = encode_ws_text_frame("");
        assert_eq!(frame[0], 0x81);
        assert_eq!(frame[1], 0);
        assert_eq!(frame.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Server lifecycle (async)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_server_starts_on_port_zero() {
        // Use port 0 to let the OS assign a free port.
        let server = DevEcoWsServer::new(0);
        let handle = server.start().await.expect("server must start");
        assert_ne!(handle.addr.port(), 0, "OS must assign a real port");
    }

    #[tokio::test]
    async fn test_broadcast_with_no_clients_returns_zero() {
        let server = DevEcoWsServer::new(0);
        let handle = server.start().await.expect("server must start");
        let count = handle.broadcast(DevEcoEvent::build_start("entry")).await;
        assert_eq!(count, 0, "no clients → broadcast count must be 0");
    }

    #[tokio::test]
    async fn test_server_handle_clone_broadcasts_independently() {
        let server = DevEcoWsServer::new(0);
        let handle = server.start().await.expect("server must start");
        let handle2 = handle.clone();
        // Both handles should broadcast without error.
        let _ = handle.broadcast(DevEcoEvent::build_start("entry")).await;
        let _ = handle2.broadcast(DevEcoEvent::build_complete("entry", true)).await;
    }

    // -----------------------------------------------------------------------
    // JSON escaping
    // -----------------------------------------------------------------------

    #[test]
    fn test_escape_json_special_chars() {
        assert_eq!(escape_json("a\"b"), "a\\\"b");
        assert_eq!(escape_json("a\\b"), "a\\\\b");
        assert_eq!(escape_json("a\nb"), "a\\nb");
    }
}
