//! A fake ACP agent for tests (`tests/acp.rs`): answers `initialize`,
//! `session/new`, `session/set_mode` and `session/prompt` the way the Claude
//! Code adapter does, after the scenario in `FAKE_ACP`:
//!
//! - `ok` (default): streams "Hello " "world" in one message, reports usage
//!   and cost, ends the turn; the prompt text `twice` sends two messages.
//! - `permission`: asks to run a tool first and answers with what it was told.
//! - `hang`: never ends a prompt until `session/cancel`.
//! - `crash`: exits in the middle of a prompt.
//! - `auth`: `session/new` fails with "Authentication required".

use std::io::{BufRead, Write};

use serde_json::{Value, json};

fn send(out: &mut impl Write, value: Value) {
    writeln!(out, "{value}").unwrap();
    out.flush().unwrap();
}

fn update(out: &mut impl Write, session: &str, update: Value) {
    send(
        out,
        json!({ "jsonrpc": "2.0", "method": "session/update",
                "params": { "sessionId": session, "update": update } }),
    );
}

fn main() {
    let scenario = std::env::var("FAKE_ACP").unwrap_or_else(|_| "ok".into());
    eprintln!("fake agent: scenario {scenario}");
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    let mut mode = "default".to_string();
    // The prompt being answered in `hang` and `permission`: its request id.
    let mut waiting: Option<Value> = None;
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let Ok(msg) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let id = msg.get("id").cloned();
        let method = msg
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match method {
            "initialize" => send(
                &mut out,
                json!({ "jsonrpc": "2.0", "id": id, "result": {
                    "protocolVersion": 1,
                    "agentCapabilities": { "loadSession": false },
                    "agentInfo": { "name": "fake", "version": "1.0" } } }),
            ),
            "session/new" if scenario == "auth" => send(
                &mut out,
                json!({ "jsonrpc": "2.0", "id": id,
                        "error": { "code": -32000, "message": "Authentication required" } }),
            ),
            "session/new" => send(
                &mut out,
                json!({ "jsonrpc": "2.0", "id": id, "result": {
                    "sessionId": "s1",
                    "modes": { "currentModeId": "default", "availableModes": [
                        { "id": "default", "name": "Manual" },
                        { "id": "acceptEdits", "name": "Accept edits" },
                        { "id": "bypassPermissions", "name": "Bypass" } ] } } }),
            ),
            "session/set_mode" => {
                mode = msg["params"]["modeId"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                send(
                    &mut out,
                    json!({ "jsonrpc": "2.0", "id": id, "result": {} }),
                );
            }
            "session/prompt" => {
                let text = msg["params"]["prompt"][0]["text"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                match scenario.as_str() {
                    "crash" => {
                        update(
                            &mut out,
                            "s1",
                            json!({ "sessionUpdate": "agent_message_chunk",
                            "content": { "type": "text", "text": "Starting" } }),
                        );
                        eprintln!("fake agent: crashing");
                        std::process::exit(3);
                    }
                    "hang" => waiting = id,
                    "permission" => {
                        update(
                            &mut out,
                            "s1",
                            json!({ "sessionUpdate": "tool_call",
                            "toolCallId": "t1", "title": "Run cargo test", "kind": "execute", "status": "pending" }),
                        );
                        send(
                            &mut out,
                            json!({ "jsonrpc": "2.0", "id": 900, "method": "session/request_permission",
                            "params": { "sessionId": "s1",
                                "toolCall": { "toolCallId": "t1", "title": "Run cargo test", "kind": "execute" },
                                "options": [
                                    { "optionId": "allow", "name": "Allow", "kind": "allow_once" },
                                    { "optionId": "reject", "name": "Reject", "kind": "reject_once" } ] } }),
                        );
                        waiting = id;
                    }
                    _ => {
                        let chunks: &[(&str, &str)] = if text == "twice" {
                            &[("m1", "First."), ("m2", "Second.")]
                        } else {
                            &[("m1", "Hello "), ("m1", "world")]
                        };
                        for (message, chunk) in chunks {
                            update(
                                &mut out,
                                "s1",
                                json!({ "sessionUpdate": "agent_message_chunk",
                                "content": { "type": "text", "text": chunk }, "messageId": message }),
                            );
                        }
                        update(
                            &mut out,
                            "s1",
                            json!({ "sessionUpdate": "usage_update", "used": 120, "size": 1000,
                            "cost": { "amount": 0.25, "currency": "USD" } }),
                        );
                        send(
                            &mut out,
                            json!({ "jsonrpc": "2.0", "id": id, "result": {
                            "stopReason": "end_turn",
                            "usage": { "inputTokens": 100, "outputTokens": 20,
                                       "cachedReadTokens": 5, "cachedWriteTokens": 7, "totalTokens": 132 },
                            "_meta": { "mode": mode } } }),
                        );
                    }
                }
            }
            "session/cancel" => {
                if let Some(id) = waiting.take() {
                    send(
                        &mut out,
                        json!({ "jsonrpc": "2.0", "id": id, "result": { "stopReason": "cancelled" } }),
                    );
                }
            }
            // The answer to our permission request.
            "" if msg.get("id").and_then(Value::as_i64) == Some(900) => {
                let outcome = &msg["result"]["outcome"];
                let chosen = outcome["optionId"].as_str().unwrap_or("cancelled");
                update(
                    &mut out,
                    "s1",
                    json!({ "sessionUpdate": "tool_call_update",
                    "toolCallId": "t1", "status": if chosen == "allow" { "completed" } else { "failed" } }),
                );
                update(
                    &mut out,
                    "s1",
                    json!({ "sessionUpdate": "agent_message_chunk",
                    "content": { "type": "text", "text": format!("permission: {chosen}") } }),
                );
                if let Some(id) = waiting.take() {
                    send(
                        &mut out,
                        json!({ "jsonrpc": "2.0", "id": id, "result": { "stopReason": "end_turn" } }),
                    );
                }
            }
            _ => {
                if let Some(id) = id {
                    send(
                        &mut out,
                        json!({ "jsonrpc": "2.0", "id": id,
                        "error": { "code": -32601, "message": "unknown" } }),
                    );
                }
            }
        }
    }
}
