use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, UdpSocket};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{Value, json};

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn stdio_tools_inspect_both_transports_and_are_read_only() -> anyhow::Result<()> {
    let tcp = TcpListener::bind("127.0.0.1:0")?;
    let address = tcp.local_addr()?;
    let _udp = UdpSocket::bind(address)?;
    let mut server = Server(
        Command::new(env!("CARGO_BIN_EXE_portspy"))
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?,
    );
    let mut input = server
        .0
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("missing stdin"))?;
    let output = server
        .0
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("missing stdout"))?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(output).lines() {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut request = |id: u32, method: &str, params: Value| -> anyhow::Result<Value> {
        writeln!(
            input,
            "{}",
            json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
        )?;
        input.flush()?;
        let message: Value = serde_json::from_str(&rx.recv_timeout(Duration::from_secs(10))??)?;
        assert_eq!(message["id"], id);
        assert!(message.get("error").is_none(), "{message}");
        Ok(message["result"].clone())
    };

    let meta = json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientCapabilities": {}
    });
    let discovery = request(1, "server/discover", json!({"_meta": meta}))?;
    assert!(
        discovery["supportedVersions"]
            .as_array()
            .is_some_and(|versions| versions.contains(&json!("2026-07-28")))
    );

    let registry = request(2, "tools/list", json!({"_meta": meta}))?;
    let tools = registry["tools"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("missing tool list"))?;
    assert_eq!(tools.len(), 2);
    assert!(tools.iter().any(|tool| tool["name"] == "list_listeners"));
    assert!(tools.iter().any(|tool| tool["name"] == "inspect_listener"));
    assert!(
        tools
            .iter()
            .all(|tool| tool["annotations"]["readOnlyHint"] == true)
    );

    let result = request(
        3,
        "tools/call",
        json!({
            "_meta": meta,
            "name": "inspect_listener",
            "arguments": {"port": address.port(), "address": address.ip()},
        }),
    )?;
    let listeners = result["structuredContent"]["listeners"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("missing listeners"))?;
    for protocol in ["tcp", "udp"] {
        let listener = listeners
            .iter()
            .find(|item| item["protocol"] == protocol)
            .ok_or_else(|| anyhow::anyhow!("missing {protocol} listener"))?;
        assert_eq!(listener["process"]["pid"], std::process::id());
        assert_eq!(listener["exposure"], "loopback");
        assert!(listener["ancestors"].is_array());
        assert!(listener["process"].get("command_line").is_none());
        assert!(listener["process"].get("working_directory").is_none());
    }
    let list = request(
        4,
        "tools/call",
        json!({"_meta": meta, "name": "list_listeners"}),
    )?;
    assert!(
        list["structuredContent"]["listeners"]
            .as_array()
            .is_some_and(|items| {
                items
                    .iter()
                    .any(|item| item["port"] == address.port() && item.get("ancestors").is_none())
            })
    );
    let udp = request(
        5,
        "tools/call",
        json!({
            "_meta": meta,
            "name": "inspect_listener",
            "arguments": {"port": address.port(), "protocol": "udp", "address": address.ip()},
        }),
    )?;
    assert!(
        udp["structuredContent"]["listeners"]
            .as_array()
            .is_some_and(|items| { items.len() == 1 && items[0]["protocol"] == "udp" })
    );
    Ok(())
}
