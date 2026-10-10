use crate::{image, library::Library, payload};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Seek},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use zip::ZipArchive;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Firmware {
    pub path: PathBuf,
    pub sha256: String,
    pub kind: String,
    pub product: String,
    pub metadata: BTreeMap<String, String>,
    pub images: Vec<PathBuf>,
    pub update_zip: Option<PathBuf>,
    pub directory: PathBuf,
}
pub fn metadata(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|l| {
            l.split_once('=')
                .map(|(k, v)| (k.trim().into(), v.trim().into()))
        })
        .collect()
}
fn wanted(name: &str) -> bool {
    matches!(
        name,
        "boot.img"
            | "init_boot.img"
            | "vendor_boot.img"
            | "vendor_kernel_boot.img"
            | "vbmeta.img"
            | "vbmeta_system.img"
            | "vbmeta_vendor.img"
            | "dtbo.img"
            | "super_empty.img"
    ) || name.starts_with("bootloader-")
        || name.starts_with("radio-")
}
fn extract<R: Read + Seek>(
    z: &mut ZipArchive<R>,
    out: &Path,
    depth: u8,
    result: &mut Firmware,
    cancel: &AtomicBool,
    log: &dyn Fn(String),
) -> Result<()> {
    ensure!(depth <= 2, "Too many nested archives");
    for i in 0..z.len() {
        ensure!(!cancel.load(Ordering::Relaxed), "Processing cancelled");
        let mut entry = z.by_index(i)?;
        let safe = entry
            .enclosed_name()
            .context("Archive contains an unsafe path")?;
        let name = safe
            .file_name()
            .context("Archive entry has no filename")?
            .to_string_lossy()
            .into_owned();
        ensure!(!entry.is_symlink(), "Archive contains symbolic links");
        if entry.is_dir() {
            continue;
        }
        ensure!(
            entry.size() <= 32 * 1024 * 1024 * 1024,
            "Archive member is too large"
        );
        if entry.name() == "META-INF/com/android/metadata" {
            ensure!(entry.size() < 1024 * 1024, "OTA metadata is too large");
            let mut text = String::new();
            entry.read_to_string(&mut text)?;
            result.metadata = metadata(&text);
            result.product = result
                .metadata
                .get("pre-device")
                .cloned()
                .unwrap_or_default();
            result.kind = "Full OTA".into();
            continue;
        }
        if wanted(&name)
            || name.starts_with("flash-all.")
            || name.starts_with("image-") && name.ends_with(".zip")
            || name == "android-info.txt"
            || name == "payload.bin"
        {
            let destination = out.join(&name);
            ensure!(
                !destination.exists(),
                "Duplicate archive member filename: {name}"
            );
            log(format!(
                "Extracting {name} ({:.1} MiB)",
                entry.size() as f64 / 1048576.0
            ));
            let mut target = File::create(&destination)?;
            std::io::copy(&mut entry, &mut target)?;
            drop(target);
            drop(entry);
            if name == "payload.bin" {
                result
                    .images
                    .extend(payload::extract(&destination, out, cancel, log)?);
                result.kind = "Full OTA".into();
            } else if name.ends_with(".zip") {
                result.update_zip = Some(destination.clone());
                let mut nested = ZipArchive::new(File::open(&destination)?)?;
                extract(&mut nested, out, depth + 1, result, cancel, log)?;
            } else if wanted(&name) {
                result.images.push(destination.clone());
            }
        }
    }
    Ok(())
}
pub fn process(
    path: &Path,
    data: &Path,
    cancel: &AtomicBool,
    log: &dyn Fn(String),
) -> Result<Firmware> {
    ensure!(path.is_file(), "Firmware file is missing");
    let (_, sha256) = image::hashes(path)?;
    let directory = data.join("firmware").join(&sha256[..16]);
    fs::create_dir_all(&directory)?;
    let mut result = Firmware {
        path: path.to_owned(),
        sha256: sha256.clone(),
        kind: "Factory image".into(),
        directory: directory.clone(),
        ..Default::default()
    };
    let manifest = directory.join("processed.json");
    if manifest.exists() {
        let cached: Firmware = serde_json::from_slice(&fs::read(&manifest)?)?;
        if cached.sha256 == sha256 && cached.images.iter().all(|p| p.is_file()) {
            return Ok(cached);
        }
    }
    // Each attempt gets a clean staging directory; failed partial extraction is never reused.
    let stage = directory.join(format!("stage-{}", chrono::Utc::now().timestamp_millis()));
    fs::create_dir(&stage)?;
    result.directory = stage.clone();
    match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "img" => {
            let info = image::inspect(path)?;
            let partition = match info.kind.as_str() {
                "boot" => "boot.img",
                "init_boot" => "init_boot.img",
                "vendor_boot" => "vendor_boot.img",
                "vbmeta" => "vbmeta.img",
                _ => path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .context("Image name is invalid")?,
            };
            let out = stage.join(partition);
            fs::copy(path, &out)?;
            result.images.push(out);
            result.kind = "Standalone image".into();
        }
        "bin" => {
            result.images = payload::extract(path, &stage, cancel, log)?;
            result.kind = "OTA payload".into();
        }
        "zip" => {
            let mut z = ZipArchive::new(File::open(path)?)?;
            extract(&mut z, &stage, 0, &mut result, cancel, log)?;
        }
        "tgz" | "gz" | "tar" => {
            let file = File::open(path)?;
            let reader: Box<dyn Read> = if path.extension().is_some_and(|v| v == "tar") {
                Box::new(file)
            } else {
                Box::new(flate2::read::GzDecoder::new(file))
            };
            let mut archive = tar::Archive::new(reader);
            for entry in archive.entries()? {
                let mut e = entry?;
                ensure!(
                    e.header().entry_type().is_file() || e.header().entry_type().is_dir(),
                    "Tar links are not supported"
                );
                let p = e.path()?.into_owned();
                ensure!(
                    !p.is_absolute()
                        && !p
                            .components()
                            .any(|c| matches!(c, std::path::Component::ParentDir)),
                    "Unsafe tar entry"
                );
                let Some(name) = p.file_name().and_then(|s| s.to_str()) else {
                    continue;
                };
                if wanted(name) {
                    let out = stage.join(name);
                    ensure!(!out.exists(), "Duplicate image filename");
                    std::io::copy(&mut e, &mut File::create(&out)?)?;
                    result.images.push(out);
                }
            }
        }
        _ => bail!("Select a factory ZIP, full OTA ZIP/payload, boot image, or tar archive"),
    }
    if result.product.is_empty() {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        result.product = name.split('-').next().unwrap_or("").into();
        if result.kind == "Standalone image" {
            result.product.clear();
        }
    }
    for p in &result.images {
        if matches!(
            p.file_name().and_then(|s| s.to_str()),
            Some("boot.img" | "init_boot.img" | "vendor_boot.img" | "vendor_kernel_boot.img")
        ) {
            let info = image::inspect(p)?;
            Library::open(data)?.insert(
                p,
                &path.to_string_lossy(),
                &result.product,
                "",
                "",
                &info,
            )?;
        }
    }
    fs::write(manifest, serde_json::to_vec_pretty(&result)?)?;
    log(format!(
        "Processed {} images; SHA-256 {}",
        result.images.len(),
        sha256
    ));
    Ok(result)
}
