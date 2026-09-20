//! A 60-line JSON-RPC client for the Bevy Remote Protocol, over its own HTTP port.
//!
//! V0-spike.md, correction 3: the agent gate is measured by talking to port 15702 directly rather than
//! by installing `bevy_brp_mcp` and registering an MCP server on this machine. BRP is a plain JSON-RPC
//! 2.0 server, so this needs no HTTP crate and no extra dependency.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// The port `bevy_remote`'s HTTP plugin listens on by default.
pub const PORT: u16 = 15702;

/// One JSON-RPC call. Returns the `result` member, or the error as text.
pub fn call(port: u16, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    })
    .to_string();
    let req = format!(
        "POST / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let mut s = TcpStream::connect(("127.0.0.1", port)).map_err(|e| format!("connect: {e}"))?;
    s.set_read_timeout(Some(Duration::from_secs(60))).ok();
    s.write_all(req.as_bytes()).map_err(|e| format!("write: {e}"))?;
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).map_err(|e| format!("read: {e}"))?;
    let text = String::from_utf8_lossy(&raw);
    let body = text
        .split_once("\r\n\r\n")
        .map(|(_, b)| b)
        .ok_or_else(|| format!("no body in response: {text}"))?;
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("bad json: {e} in {body}"))?;
    if let Some(err) = v.get("error") {
        return Err(err.to_string());
    }
    v.get("result")
        .cloned()
        .ok_or_else(|| format!("no result: {v}"))
}

/// Polls until the server answers `rpc.discover`, or the deadline passes.
pub fn wait_ready(port: u16, secs: u64) -> Result<u32, String> {
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    let mut tries = 0;
    loop {
        tries += 1;
        match call(port, "rpc.discover", serde_json::Value::Null) {
            Ok(_) => return Ok(tries),
            Err(e) if std::time::Instant::now() >= deadline => {
                return Err(format!("not ready after {tries} tries: {e}"))
            }
            Err(_) => std::thread::sleep(Duration::from_millis(250)),
        }
    }
}
