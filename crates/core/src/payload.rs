use anyhow::{Context, Result, bail, ensure};
use prost::Message;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, PartialEq, Message)]
pub struct Manifest {
    #[prost(uint32, optional, tag = "3")]
    pub block_size: Option<u32>,
    #[prost(uint32, optional, tag = "12")]
    pub minor_version: Option<u32>,
    #[prost(message, repeated, tag = "13")]
    pub partitions: Vec<Partition>,
}
#[derive(Clone, PartialEq, Message)]
pub struct Partition {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(message, optional, tag = "7")]
    pub info: Option<PartitionInfo>,
    #[prost(message, repeated, tag = "8")]
    pub operations: Vec<Operation>,
}
#[derive(Clone, PartialEq, Message)]
pub struct PartitionInfo {
    #[prost(uint64, optional, tag = "1")]
    pub size: Option<u64>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub hash: Option<Vec<u8>>,
}
#[derive(Clone, PartialEq, Message)]
pub struct Operation {
    #[prost(int32, tag = "1")]
    pub kind: i32,
    #[prost(uint64, optional, tag = "2")]
    pub offset: Option<u64>,
    #[prost(uint64, optional, tag = "3")]
    pub length: Option<u64>,
    #[prost(message, repeated, tag = "6")]
    pub extents: Vec<Extent>,
    #[prost(bytes = "vec", optional, tag = "8")]
    pub hash: Option<Vec<u8>>,
}
#[derive(Clone, PartialEq, Message)]
pub struct Extent {
    #[prost(uint64, optional, tag = "1")]
    pub start: Option<u64>,
    #[prost(uint64, optional, tag = "2")]
    pub blocks: Option<u64>,
}
fn u64be(reader: &mut impl Read) -> Result<u64> {
    let mut b = [0; 8];
    reader.read_exact(&mut b)?;
    Ok(u64::from_be_bytes(b))
}
pub fn read_manifest(file: &mut File) -> Result<(Manifest, u64)> {
    file.seek(SeekFrom::Start(0))?;
    let mut magic = [0; 4];
    file.read_exact(&mut magic)?;
    ensure!(&magic == b"CrAU", "Not an Android update payload");
    let version = u64be(file)?;
    ensure!(
        version == 1 || version == 2,
        "Unsupported payload version {version}"
    );
    let length = u64be(file)?;
    ensure!(length <= 64 * 1024 * 1024, "Payload manifest is too large");
    let signatures = if version == 2 {
        let mut b = [0; 4];
        file.read_exact(&mut b)?;
        u32::from_be_bytes(b) as u64
    } else {
        0
    };
    let mut bytes = vec![0; length as usize];
    file.read_exact(&mut bytes)?;
    let manifest = Manifest::decode(bytes.as_slice())?;
    ensure!(
        manifest.minor_version.unwrap_or(0) == 0,
        "Incremental/delta OTA needs old partitions and is not supported by the full-OTA extractor"
    );
    let offset = file
        .stream_position()?
        .checked_add(signatures)
        .context("Payload offset overflow")?;
    ensure!(
        offset <= file.metadata()?.len(),
        "Truncated payload signature block"
    );
    Ok((manifest, offset))
}
pub fn extract(
    path: &Path,
    out: &Path,
    cancel: &AtomicBool,
    log: &dyn Fn(String),
) -> Result<Vec<PathBuf>> {
    let mut file = File::open(path)?;
    let total = file.metadata()?.len();
    let (manifest, data_offset) = read_manifest(&mut file)?;
    let block = u64::from(manifest.block_size.unwrap_or(4096));
    ensure!(
        (512..=65536).contains(&block) && block.is_power_of_two(),
        "Invalid payload block size"
    );
    let mut outputs = vec![];
    for p in &manifest.partitions {
        if !matches!(
            p.name.as_str(),
            "boot"
                | "init_boot"
                | "vendor_boot"
                | "vendor_kernel_boot"
                | "vbmeta"
                | "vbmeta_system"
                | "vbmeta_vendor"
                | "dtbo"
                | "super_empty"
        ) {
            continue;
        }
        let expected_size = p
            .info
            .as_ref()
            .and_then(|v| v.size)
            .context("Partition size is missing")?;
        ensure!(
            expected_size <= 4 * 1024 * 1024 * 1024,
            "Partition exceeds extraction limit"
        );
        let destination = out.join(format!("{}.img", p.name));
        ensure!(!destination.exists(), "Duplicate payload image {}", p.name);
        let mut output = File::create(&destination)?;
        output.set_len(expected_size)?;
        log(format!(
            "Extracting payload partition {} ({} operations)",
            p.name,
            p.operations.len()
        ));
        for op in &p.operations {
            ensure!(!cancel.load(Ordering::Relaxed), "Extraction cancelled");
            let mut expected = 0u64;
            for e in &op.extents {
                let size = e
                    .blocks
                    .unwrap_or(0)
                    .checked_mul(block)
                    .context("Extent overflow")?;
                expected = expected
                    .checked_add(size)
                    .context("Extent length overflow")?;
                let begin = e
                    .start
                    .unwrap_or(0)
                    .checked_mul(block)
                    .context("Extent offset overflow")?;
                ensure!(
                    begin.checked_add(size).is_some_and(|n| n <= expected_size),
                    "Payload extent outside partition"
                );
            }
            ensure!(
                expected <= 512 * 1024 * 1024,
                "Payload operation exceeds memory limit"
            );
            if op.kind == 6 || op.kind == 7 {
                for e in &op.extents {
                    output.seek(SeekFrom::Start(e.start.unwrap_or(0) * block))?;
                    let mut remaining = e.blocks.unwrap_or(0) * block;
                    let zeros = [0; 65536];
                    while remaining > 0 {
                        let n = remaining.min(zeros.len() as u64) as usize;
                        output.write_all(&zeros[..n])?;
                        remaining -= n as u64;
                    }
                }
                continue;
            }
            ensure!(
                matches!(op.kind, 0 | 1 | 8),
                "Unsupported delta payload operation {}",
                op.kind
            );
            let length = op.length.unwrap_or(0);
            ensure!(
                length <= 512 * 1024 * 1024,
                "Compressed operation exceeds memory limit"
            );
            let begin = data_offset
                .checked_add(op.offset.unwrap_or(0))
                .context("Payload offset overflow")?;
            ensure!(
                begin.checked_add(length).is_some_and(|n| n <= total),
                "Payload operation outside archive"
            );
            file.seek(SeekFrom::Start(begin))?;
            let mut encoded = vec![0; length as usize];
            file.read_exact(&mut encoded)?;
            if let Some(hash) = &op.hash {
                ensure!(
                    Sha256::digest(&encoded).as_slice() == hash.as_slice(),
                    "Payload operation SHA-256 mismatch"
                );
            }
            let mut decoded = vec![];
            match op.kind {
                0 => decoded = encoded,
                1 => {
                    bzip2::read::BzDecoder::new(encoded.as_slice())
                        .take(expected + 1)
                        .read_to_end(&mut decoded)?;
                }
                8 => {
                    xz2::read::XzDecoder::new(encoded.as_slice())
                        .take(expected + 1)
                        .read_to_end(&mut decoded)?;
                }
                _ => unreachable!(),
            }
            ensure!(
                decoded.len() as u64 == expected,
                "Decoded payload operation length mismatch"
            );
            let mut cursor = 0usize;
            for e in &op.extents {
                let n = (e.blocks.unwrap_or(0) * block) as usize;
                output.seek(SeekFrom::Start(e.start.unwrap_or(0) * block))?;
                output.write_all(&decoded[cursor..cursor + n])?;
                cursor += n;
            }
        }
        output.sync_all()?;
        drop(output);
        if let Some(hash) = p.info.as_ref().and_then(|v| v.hash.as_ref()) {
            let (_, actual) = crate::image::hashes(&destination)?;
            if actual != hex::encode(hash) {
                let _ = fs::remove_file(&destination);
                bail!("Extracted {} partition SHA-256 mismatch", p.name);
            }
        }
        outputs.push(destination);
    }
    ensure!(
        !outputs.is_empty(),
        "Payload contains no supported boot/metadata partitions"
    );
    Ok(outputs)
}
