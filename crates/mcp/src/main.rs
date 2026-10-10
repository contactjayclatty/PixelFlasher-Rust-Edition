use std::io::{self, BufRead, Write};
fn main() {
    let data = pixelflasher_core::config::data_dir();
    let mut input = io::BufReader::new(io::stdin());
    let mut output = io::stdout().lock();
    loop {
        let mut bytes = vec![];
        // Bound memory before parsing hostile client input (1 MiB per message).
        let read = std::io::Read::take(&mut input, 1_048_577).read_until(b'\n', &mut bytes);
        if !matches!(read,Ok(n)if n>0) {
            break;
        }
        if bytes.len() > 1_048_576 {
            break;
        }
        let response = match serde_json::from_slice(&bytes) {
            Ok(request) => pixelflasher_core::agent::protocol(&data, request),
            Err(_) => Some(
                serde_json::json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Invalid JSON"}}),
            ),
        };
        if let Some(value) = response {
            if writeln!(output, "{value}").is_err() {
                break;
            }
            let _ = output.flush();
        }
    }
}
