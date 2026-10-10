use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageInfo {
    pub name: String,
    pub size: u64,
    pub sha1: String,
    pub sha256: String,
    pub kind: String,
    pub header_version: Option<u32>,
    pub kernel_size: Option<u32>,
    pub ramdisk_size: Option<u32>,
    pub os_version: String,
    pub security_patch: String,
    pub avb: Option<AvbInfo>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AvbInfo {
    pub offset: u64,
    pub size: u64,
    pub algorithm: u32,
    pub rollback_index: u64,
    pub flags: u32,
    pub release: String,
    pub original_size: Option<u64>,
    #[serde(default)]
    pub properties: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub descriptor_count: usize,
}
pub fn avb_properties(data: &[u8]) -> Result<(std::collections::BTreeMap<String, String>, usize)> {
    let mut properties = std::collections::BTreeMap::new();
    let mut cursor = 0usize;
    let mut count = 0;
    while cursor < data.len() {
        ensure!(data.len() - cursor >= 16, "Truncated AVB descriptor");
        let chunk = &data[cursor..];
        let tag = be64(chunk, 0).unwrap();
        let body = usize::try_from(be64(chunk, 8).unwrap())?;
        let total = body
            .checked_add(16)
            .ok_or_else(|| anyhow::anyhow!("AVB descriptor length overflow"))?;
        ensure!(
            total <= chunk.len() && total.is_multiple_of(8),
            "Invalid AVB descriptor size"
        );
        if tag == 0 {
            ensure!(total >= 32, "Truncated AVB property header");
            let key = usize::try_from(be64(chunk, 16).unwrap())?;
            let value = usize::try_from(be64(chunk, 24).unwrap())?;
            let value_start = 32usize
                .checked_add(key)
                .and_then(|v| v.checked_add(1))
                .ok_or_else(|| anyhow::anyhow!("AVB property length overflow"))?;
            let end = value_start
                .checked_add(value)
                .ok_or_else(|| anyhow::anyhow!("AVB property length overflow"))?;
            ensure!(
                end < total && chunk[value_start - 1] == 0 && chunk[end] == 0,
                "Invalid AVB property string boundaries"
            );
            let key = std::str::from_utf8(&chunk[32..value_start - 1])?.to_owned();
            let bytes = &chunk[value_start..end];
            let value = std::str::from_utf8(bytes)
                .map(str::to_owned)
                .unwrap_or_else(|_| format!("hex:{}", hex::encode(bytes)));
            ensure!(
                properties.insert(key, value).is_none(),
                "Duplicate AVB property key"
            );
        }
        cursor += total;
        count += 1;
    }
    Ok((properties, count))
}
pub fn hashes(path: &Path) -> Result<(String, String)> {
    let mut f = File::open(path)?;
    let mut a = Sha1::new();
    let mut b = Sha256::new();
    let mut buf = [0u8; 128 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        a.update(&buf[..n]);
        b.update(&buf[..n]);
    }
    Ok((hex::encode(a.finalize()), hex::encode(b.finalize())))
}
fn le32(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        data.get(offset..offset + 4)?.try_into().ok()?,
    ))
}
fn be32(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        data.get(offset..offset + 4)?.try_into().ok()?,
    ))
}
fn be64(data: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_be_bytes(
        data.get(offset..offset + 8)?.try_into().ok()?,
    ))
}
pub fn inspect(path: &Path) -> Result<ImageInfo> {
    let size = std::fs::metadata(path)?.len();
    ensure!(size > 0, "Image is empty");
    let (sha1, sha256) = hashes(path)?;
    let mut file = File::open(path)?;
    let mut header = vec![0; 4096.min(size as usize)];
    file.read_exact(&mut header)?;
    let mut info = ImageInfo {
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        size,
        sha1,
        sha256,
        kind: "Raw partition image".into(),
        ..Default::default()
    };
    if header.starts_with(b"ANDROID!") {
        ensure!(header.len() >= 48, "Truncated Android boot header");
        let version = le32(&header, 40).unwrap();
        ensure!(
            version <= 4,
            "Unsupported Android boot header version {version}"
        );
        info.header_version = Some(version);
        info.kernel_size = le32(&header, 8);
        info.ramdisk_size = le32(&header, if version >= 3 { 12 } else { 16 });
        info.kind = if info.kernel_size == Some(0) && version >= 4 {
            "init_boot"
        } else {
            "boot"
        }
        .into();
        let packed = le32(&header, if version >= 3 { 16 } else { 44 }).unwrap_or_default();
        let os = packed >> 11;
        info.os_version = format!("{}.{}.{}", (os >> 14) & 127, (os >> 7) & 127, os & 127);
        let patch = packed & 2047;
        if patch > 0 {
            info.security_patch = format!("{}-{:02}", 2000 + (patch >> 4), patch & 15);
        }
        let kernel = u64::from(info.kernel_size.unwrap_or_default());
        let ramdisk = u64::from(info.ramdisk_size.unwrap_or_default());
        ensure!(
            kernel.checked_add(ramdisk).is_some_and(|n| n <= size),
            "Boot components exceed file size"
        );
    } else if header.starts_with(b"VNDRBOOT") {
        ensure!(header.len() >= 28, "Truncated vendor boot header");
        info.kind = "vendor_boot".into();
        info.header_version = le32(&header, 8);
        info.ramdisk_size = le32(&header, 24);
    } else if header.starts_with(b"AVB0") {
        info.kind = "vbmeta".into();
    } else if header.starts_with(&[0x3a, 0xff, 0x26, 0xed]) {
        info.kind = "Android sparse image".into();
    }
    let mut avb_offset = if header.starts_with(b"AVB0") {
        Some((0, size, None))
    } else {
        None
    };
    if size >= 64 {
        file.seek(SeekFrom::End(-64))?;
        let mut footer = [0; 64];
        file.read_exact(&mut footer)?;
        if &footer[..4] == b"AVBf" {
            let offset = be64(&footer, 20).unwrap();
            let bytes = be64(&footer, 28).unwrap();
            ensure!(
                bytes >= 256 && offset.checked_add(bytes).is_some_and(|n| n <= size - 64),
                "AVB footer points outside the image"
            );
            avb_offset = Some((offset, bytes, be64(&footer, 12)));
        }
    }
    if let Some((offset, bytes, original_size)) = avb_offset {
        ensure!(bytes >= 256, "Truncated AVB header");
        file.seek(SeekFrom::Start(offset))?;
        let mut avb = [0u8; 256];
        file.read_exact(&mut avb)?;
        ensure!(&avb[..4] == b"AVB0", "Invalid AVB metadata magic");
        let auth = be64(&avb, 12).unwrap();
        let auxiliary = be64(&avb, 20).unwrap();
        let descriptor_offset = be64(&avb, 96).unwrap();
        let descriptor_size = be64(&avb, 104).unwrap();
        ensure!(
            256u64
                .checked_add(auth)
                .and_then(|v| v.checked_add(auxiliary))
                .is_some_and(|v| v <= bytes),
            "AVB authentication/auxiliary blocks exceed metadata size"
        );
        ensure!(
            descriptor_offset
                .checked_add(descriptor_size)
                .is_some_and(|v| v <= auxiliary)
                && descriptor_size <= 16 * 1024 * 1024,
            "AVB descriptors exceed auxiliary block or size limit"
        );
        file.seek(SeekFrom::Start(offset + 256 + auth + descriptor_offset))?;
        let mut descriptors = vec![0; descriptor_size as usize];
        file.read_exact(&mut descriptors)?;
        let (properties, descriptor_count) = avb_properties(&descriptors)?;
        info.avb = Some(AvbInfo {
            offset,
            size: bytes,
            algorithm: be32(&avb, 28).unwrap(),
            rollback_index: be64(&avb, 112).unwrap(),
            flags: be32(&avb, 120).unwrap(),
            release: String::from_utf8_lossy(&avb[128..176])
                .trim_end_matches('\0')
                .into(),
            original_size,
            properties,
            descriptor_count,
        });
    }
    Ok(info)
}
