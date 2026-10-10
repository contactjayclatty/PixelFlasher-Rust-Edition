use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

fn u16le(b: &[u8], o: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        b.get(o..o + 2)
            .context("Truncated binary XML")?
            .try_into()?,
    ))
}
fn u32le(b: &[u8], o: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        b.get(o..o + 4)
            .context("Truncated binary XML")?
            .try_into()?,
    ))
}
fn length8(b: &[u8], pos: &mut usize) -> Result<usize> {
    let n = *b.get(*pos).context("Truncated string length")?;
    *pos += 1;
    if n & 128 != 0 {
        let low = *b.get(*pos).context("Truncated string length")?;
        *pos += 1;
        Ok(((n & 127) as usize) << 8 | low as usize)
    } else {
        Ok(n as usize)
    }
}
fn length16(b: &[u8], pos: &mut usize) -> Result<usize> {
    let n = u16le(b, *pos)?;
    *pos += 2;
    if n & 0x8000 != 0 {
        let low = u16le(b, *pos)?;
        *pos += 2;
        Ok(((n & 0x7fff) as usize) << 16 | low as usize)
    } else {
        Ok(n as usize)
    }
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
pub fn decode(bytes: &[u8]) -> Result<String> {
    if bytes.starts_with(b"<") || bytes.starts_with(b"<?xml") {
        return Ok(String::from_utf8(bytes.to_vec())?);
    }
    ensure!(u16le(bytes, 0)? == 3, "Not Android binary XML");
    let declared = u32le(bytes, 4)? as usize;
    ensure!(
        declared <= bytes.len() && declared >= 8,
        "Invalid binary XML size"
    );
    let mut offset = u16le(bytes, 2)? as usize;
    let mut strings = vec![];
    let mut output = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    let mut depth = 0usize;
    let mut namespaces = std::collections::BTreeMap::new();
    while offset < declared {
        let kind = u16le(bytes, offset)?;
        let header = u16le(bytes, offset + 2)? as usize;
        let size = u32le(bytes, offset + 4)? as usize;
        ensure!(
            size >= header && size >= 8 && offset.checked_add(size).is_some_and(|v| v <= declared),
            "Invalid XML chunk size"
        );
        let chunk = &bytes[offset..offset + size];
        match kind {
            1 => {
                ensure!(header >= 28, "Invalid string pool header");
                let count = u32le(chunk, 8)? as usize;
                ensure!(count < 1_000_000, "String pool is too large");
                let flags = u32le(chunk, 16)?;
                let start = u32le(chunk, 20)? as usize;
                for n in 0..count {
                    let mut pos = start
                        .checked_add(u32le(chunk, header + n * 4)? as usize)
                        .context("String offset overflow")?;
                    let string = if flags & 0x100 != 0 {
                        let _ = length8(chunk, &mut pos)?;
                        let length = length8(chunk, &mut pos)?;
                        String::from_utf8(
                            chunk
                                .get(pos..pos + length)
                                .context("String outside pool")?
                                .to_vec(),
                        )?
                    } else {
                        let length = length16(chunk, &mut pos)?;
                        let data = chunk
                            .get(pos..pos + length * 2)
                            .context("UTF16 string outside pool")?;
                        String::from_utf16(
                            &data
                                .as_chunks::<2>()
                                .0
                                .iter()
                                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                                .collect::<Vec<_>>(),
                        )?
                    };
                    strings.push(string);
                }
            }
            0x100 => {
                let prefix = u32le(chunk, 16)?;
                let uri = u32le(chunk, 20)?;
                let p = strings
                    .get(prefix as usize)
                    .context("Invalid namespace prefix")?;
                let u = strings.get(uri as usize).context("Invalid namespace URI")?;
                namespaces.insert(u.clone(), p.clone());
            }
            0x102 => {
                let index = u32le(chunk, 20)? as usize;
                let tag = strings.get(index).context("Invalid element name")?;
                output.push_str(&format!("{}<{tag}", "  ".repeat(depth)));
                if depth == 0 {
                    for (uri, prefix) in &namespaces {
                        output.push_str(&format!(" xmlns:{prefix}=\"{}\"", escape(uri)));
                    }
                }
                let begin = 16 + u16le(chunk, 24)? as usize;
                let attrsize = u16le(chunk, 26)? as usize;
                let count = u16le(chunk, 28)? as usize;
                ensure!(attrsize >= 20, "Invalid XML attribute size");
                for n in 0..count {
                    let a = begin + n * attrsize;
                    let ns = u32le(chunk, a)?;
                    let name = strings
                        .get(u32le(chunk, a + 4)? as usize)
                        .context("Invalid attribute name")?;
                    let raw = u32le(chunk, a + 8)?;
                    let kind = *chunk.get(a + 15).context("Invalid typed value")?;
                    let data = u32le(chunk, a + 16)?;
                    let value = if raw != u32::MAX {
                        strings
                            .get(raw as usize)
                            .context("Invalid string reference")?
                            .clone()
                    } else {
                        match kind {
                            3 => strings
                                .get(data as usize)
                                .context("Invalid typed string")?
                                .clone(),
                            0x12 => (data != 0).to_string(),
                            0x10 => data.to_string(),
                            0x11 => format!("0x{data:x}"),
                            1 => format!("@0x{data:08x}"),
                            2 => format!("?0x{data:08x}"),
                            4 => f32::from_bits(data).to_string(),
                            _ => format!("0x{data:08x}"),
                        }
                    };
                    let prefix = if ns != u32::MAX {
                        strings
                            .get(ns as usize)
                            .and_then(|u| namespaces.get(u))
                            .map(|p| format!("{p}:"))
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };
                    output.push_str(&format!(" {prefix}{name}=\"{}\"", escape(&value)));
                }
                output.push_str(">\n");
                depth += 1;
            }
            0x103 => {
                depth = depth.checked_sub(1).context("Unbalanced XML end tag")?;
                let tag = strings
                    .get(u32le(chunk, 20)? as usize)
                    .context("Invalid end tag")?;
                output.push_str(&format!("{}</{tag}>\n", "  ".repeat(depth)));
            }
            0x104 => {
                let text = strings
                    .get(u32le(chunk, 16)? as usize)
                    .context("Invalid text node")?;
                output.push_str(&format!("{}{}\n", "  ".repeat(depth), escape(text)));
            }
            _ => {}
        }
        offset += size;
    }
    ensure!(depth == 0, "Unclosed XML elements");
    Ok(output)
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BuildPrint {
    pub brand: String,
    pub manufacturer: String,
    pub model: String,
    pub device: String,
    pub product: String,
    pub fingerprint: String,
    pub security_patch: String,
    pub first_api_level: String,
}
pub fn build_print(text: &str) -> BuildPrint {
    let props: std::collections::BTreeMap<_, _> = text
        .lines()
        .filter_map(|l| l.split_once('=').map(|(k, v)| (k.trim(), v.trim())))
        .collect();
    let get = |keys: &[&str]| {
        keys.iter()
            .find_map(|k| props.get(k))
            .copied()
            .unwrap_or("")
            .to_owned()
    };
    BuildPrint {
        brand: get(&["ro.product.brand"]),
        manufacturer: get(&["ro.product.manufacturer"]),
        model: get(&["ro.product.model"]),
        device: get(&["ro.product.device"]),
        product: get(&["ro.product.name"]),
        fingerprint: get(&["ro.build.fingerprint", "ro.system.build.fingerprint"]),
        security_patch: get(&["ro.build.version.security_patch"]),
        first_api_level: get(&["ro.product.first_api_level"]),
    }
}
