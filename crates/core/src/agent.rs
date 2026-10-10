//! Local planning boundary. Deliberately has no transport or execution API.
use crate::{
    config::Config, library::Library, operation::OperationIntent, providers, root::Solution,
};
use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::Path;
pub fn device_id(serial: &str) -> String {
    format!("device-{}", hex::encode(Sha256::digest(serial.as_bytes())))
}
pub fn tools() -> Value {
    json!({"tools":[
     {"name":"inspect","description":"Inspect saved device resources and image metadata; snapshots may be stale","inputSchema":{"type":"object","properties":{},"additionalProperties":false}},
     {"name":"capabilities","description":"Pinned root adapter capabilities; hardware remains unverified","inputSchema":{"type":"object","properties":{"provider":{"type":"string","enum":["Magisk","KernelSu","KernelSuNext","SukiSu","WildKsu","APatch"]},"version":{"type":"string"}},"required":["provider","version"],"additionalProperties":false}},
     {"name":"prepare","description":"Queue an intent for fresh local desktop review; cannot approve or execute","inputSchema":{"type":"object","properties":{"intent":{"oneOf":[{"type":"object","properties":{"kind":{"const":"OpenManager"},"device_id":{"type":"string"},"provider":{"enum":["Magisk","KernelSu","KernelSuNext","SukiSu","WildKsu","APatch"]}},"required":["kind","device_id","provider"],"additionalProperties":false},{"type":"object","properties":{"kind":{"const":"FlashImage"},"device_id":{"type":"string"},"artifact_id":{"type":"string"},"partition":{"enum":["boot","init_boot","vendor_boot"]}},"required":["kind","device_id","artifact_id","partition"],"additionalProperties":false}]}},"required":["intent"],"additionalProperties":false}},
     {"name":"status","description":"Journal status without serials, commands or secrets","inputSchema":{"type":"object","properties":{},"additionalProperties":false}}
    ]})
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Prepare {
    intent: OperationIntent,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Capabilities {
    provider: Solution,
    version: String,
}
pub fn call(data: &Path, name: &str, args: Value) -> Result<Value> {
    let config = Config::load(data)?;
    ensure!(
        config.mcp_enabled,
        "MCP_DISABLED: Enable local integration in desktop Settings first"
    );
    let library = Library::open(data)?;
    match name {
        "inspect" => {
            ensure!(args == json!({}), "INVALID_ARGUMENTS");
            Ok(
                json!({"devices":config.devices.iter().filter(|d|d.enabled).map(|d|json!({"id":device_id(&d.serial),"label":"Saved device","freshness":"saved; inspect locally before review"})).collect::<Vec<_>>(),"artifacts":library.list()?.iter().map(|a|json!({"id":format!("image-{}",a.info.sha256),"sha256":a.info.sha256,"kind":a.info.kind,"patched":!a.patched_with.is_empty()})).collect::<Vec<_>>()}),
            )
        }
        "capabilities" => {
            let a: Capabilities = serde_json::from_value(args)?;
            Ok(serde_json::to_value(
                providers::provider(a.provider).capabilities(&a.version),
            )?)
        }
        "status" => {
            ensure!(args == json!({}), "INVALID_ARGUMENTS");
            Ok(json!(library.recent_operations()?.into_iter().filter(|r|r.id.strip_prefix("op-").is_some_and(|v|v.len()==24&&v.chars().all(|c|c.is_ascii_hexdigit()))).map(|r|json!({"id":r.id,"status":if ["Prepared","Running","CommandsCompleted","Verified","Failed","Cancelled","Interrupted"].contains(&r.status.as_str()){r.status.as_str()}else{"Unknown"}})).collect::<Vec<_>>()))
        }
        "prepare" => {
            let request: Prepare = serde_json::from_value(args)?;
            let device = match &request.intent {
                OperationIntent::OpenManager { device_id, .. }
                | OperationIntent::FlashImage { device_id, .. } => device_id,
                OperationIntent::PatchRoot { .. } => {
                    bail!("UNSUPPORTED: Root patching requires local APK and secret selection")
                }
            };
            ensure!(
                config
                    .devices
                    .iter()
                    .any(|d| d.enabled && device_id(&d.serial) == *device),
                "UNKNOWN_DEVICE"
            );
            if let OperationIntent::FlashImage {
                artifact_id,
                partition,
                ..
            } = &request.intent
            {
                ensure!(
                    ["boot", "init_boot", "vendor_boot"].contains(&partition.as_str()),
                    "UNSUPPORTED_PARTITION"
                );
                ensure!(
                    library
                        .list()?
                        .iter()
                        .any(|a| format!("image-{}", a.info.sha256) == *artifact_id
                            && a.info.kind == *partition),
                    "UNKNOWN_OR_INCOMPATIBLE_ARTIFACT"
                );
            }
            ensure!(
                library.review_requests()?.len() < 100,
                "REVIEW_QUEUE_FULL: Dismiss or review pending requests locally"
            );
            let id = library.enqueue_review(&request.intent)?;
            Ok(
                json!({"id":id,"status":"Pending local inspection and human review","execution_allowed":false}),
            )
        }
        _ => bail!(
            "FORBIDDEN_TOOL: This server cannot approve, execute, access secrets or invoke commands"
        ),
    }
}
pub fn protocol(data: &Path, request: Value) -> Option<Value> {
    let id = request.get("id")?.clone();
    let response = (|| -> Result<Value> {
        ensure!(
            request.get("jsonrpc") == Some(&json!("2.0")),
            "Invalid JSON-RPC version"
        );
        match request.get("method").and_then(Value::as_str).unwrap_or("") {
            "initialize" => Ok(
                json!({"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"pixelflasher-mcp","version":env!("CARGO_PKG_VERSION")}}),
            ),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(tools()),
            "tools/call" => {
                let params = request
                    .get("params")
                    .ok_or_else(|| anyhow::anyhow!("Missing params"))?;
                ensure!(
                    params
                        .as_object()
                        .is_some_and(|m| m.keys().all(|k| k == "name" || k == "arguments")),
                    "Unknown call fields"
                );
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                match call(
                    data,
                    name,
                    params.get("arguments").cloned().unwrap_or(json!({})),
                ) {
                    Ok(value) => Ok(
                        json!({"content":[{"type":"text","text":serde_json::to_string(&value)?}],"structuredContent":{"result":value},"isError":false}),
                    ),
                    Err(e) => {
                        Ok(json!({"content":[{"type":"text","text":e.to_string()}],"isError":true}))
                    }
                }
            }
            _ => bail!("Unknown method"),
        }
    })();
    Some(match response {
        Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
        Err(e) => json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":e.to_string()}}),
    })
}
