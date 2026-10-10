use crate::{
    agent,
    command::Invocation,
    device::{Device, Mode},
    flash::Plan,
    image,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartitionRecord {
    pub partition: String,
    pub file: String,
    pub sha256: String,
    pub size: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupManifest {
    pub schema: u32,
    pub device_id: String,
    pub product: String,
    pub build: String,
    pub slot: String,
    pub created: String,
    pub partitions: Vec<PartitionRecord>,
}
#[derive(Clone)]
pub struct BackupOutput {
    pub directory: PathBuf,
    pub device: Device,
    pub partitions: Vec<String>,
}
pub struct BackupJob {
    pub plan: Plan,
    pub output: BackupOutput,
}
pub fn prepare(d: &Device, partitions: &[String], directory: &Path) -> Result<BackupJob> {
    ensure!(
        d.mode == Mode::Adb && d.rooted,
        "Multi-partition backup requires root in Android mode"
    );
    ensure!(
        !d.product.is_empty() && !partitions.is_empty(),
        "Inspect device and select partitions"
    );
    ensure!(
        directory.is_dir(),
        "Choose an existing empty backup directory"
    );
    ensure!(
        fs::read_dir(directory)?.next().is_none(),
        "Backup directory must be empty"
    );
    let mut seen = std::collections::BTreeSet::new();
    let mut plan = Plan::new("Back up selected partitions with checksums", d);
    for name in partitions {
        crate::command::validate_identifier(name)?;
        ensure!(seen.insert(name), "Duplicate partition");
        let remote = format!(
            "/data/local/tmp/{}_{}",
            crate::operation::opaque_id("backup"),
            name
        );
        let read = format!(
            "dd if=/dev/block/by-name/{name} of={remote}.img bs=1048576 && sha256sum {remote}.img > {remote}.sha256 && chmod 644 {remote}.img {remote}.sha256"
        );
        plan.add(Invocation::adb(
            &d.serial,
            &["shell", "su", "-c", &read],
            "Read partition and hash source copy",
        ));
        for suffix in ["img", "sha256"] {
            plan.add(Invocation::adb(
                &d.serial,
                &[
                    "pull",
                    &format!("{remote}.{suffix}"),
                    &directory
                        .join(format!("{name}.{suffix}"))
                        .display()
                        .to_string(),
                ],
                "Retrieve backup and checksum",
            ));
        }
    }
    plan.warnings.push("Backup hashes verify transfer integrity, not firmware trust. Root data is backed up separately. This operation does not flash partitions.".into());
    Ok(BackupJob {
        plan,
        output: BackupOutput {
            directory: directory.into(),
            device: d.clone(),
            partitions: partitions.into(),
        },
    })
}
pub fn finalize(output: &BackupOutput) -> Result<BackupManifest> {
    let mut records = vec![];
    for name in &output.partitions {
        crate::command::validate_identifier(name)?;
        let path = output.directory.join(format!("{name}.img"));
        let hash = image::hashes(&path)?.1;
        let expected = fs::read_to_string(output.directory.join(format!("{name}.sha256")))?;
        let expected = expected
            .split_whitespace()
            .next()
            .context("Missing source checksum")?;
        ensure!(
            expected.len() == 64 && expected.eq_ignore_ascii_case(&hash),
            "Backup transfer checksum mismatch for {name}"
        );
        let size = fs::metadata(&path)?.len();
        ensure!(size > 0, "Empty backup");
        records.push(PartitionRecord {
            partition: name.clone(),
            file: format!("{name}.img"),
            sha256: hash,
            size,
        });
    }
    let d = &output.device;
    let manifest = BackupManifest {
        schema: 1,
        device_id: agent::device_id(&d.serial),
        product: d.product.clone(),
        build: d.build.clone(),
        slot: d.slot.clone(),
        created: chrono::Utc::now().to_rfc3339(),
        partitions: records,
    };
    use std::io::Write;
    let mut out = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(output.directory.join("backup.json"))?;
    out.write_all(&serde_json::to_vec_pretty(&manifest)?)?;
    out.sync_all()?;
    Ok(manifest)
}
pub fn verify(directory: &Path, d: &Device) -> Result<BackupManifest> {
    let manifest_path = directory.join("backup.json");
    ensure!(
        fs::metadata(&manifest_path)?.len() < 1024 * 1024,
        "Backup manifest is too large"
    );
    let manifest: BackupManifest = serde_json::from_slice(&fs::read(manifest_path)?)?;
    ensure!(
        manifest.schema == 1 && !manifest.partitions.is_empty(),
        "Unsupported or empty backup manifest"
    );
    ensure!(
        manifest.device_id == agent::device_id(&d.serial) && manifest.product == d.product,
        "Backup belongs to another device"
    );
    ensure!(
        !manifest.build.is_empty() && manifest.build == d.build,
        "Backup build differs or is unknown; restore is unavailable"
    );
    ensure!(
        manifest.slot == d.slot && !d.slot.is_empty(),
        "Backup slot differs or is unknown; inspect recovery state"
    );
    let mut names = std::collections::BTreeSet::new();
    for item in &manifest.partitions {
        crate::command::validate_identifier(&item.partition)?;
        ensure!(
            names.insert(&item.partition) && item.file == format!("{}.img", item.partition),
            "Invalid backup file mapping"
        );
        let path = directory.join(&item.file);
        ensure!(
            !fs::symlink_metadata(&path)?.file_type().is_symlink(),
            "Backup symlinks are not supported"
        );
        ensure!(
            fs::metadata(&path)?.len() == item.size && image::hashes(&path)?.1 == item.sha256,
            "Backup content changed"
        );
    }
    Ok(manifest)
}
pub fn restore_plan(directory: &Path, d: &Device) -> Result<Plan> {
    ensure!(
        d.mode == Mode::Adb,
        "Matching restore currently requires fresh Android inspection; bootloop recovery remains unsupported"
    );
    let manifest = verify(directory, d)?;
    let mut plan = Plan::new("Restore verified matching partition backup", d);
    plan.destructive = true;
    plan.requires_unlocked = true;
    plan.expected_build = Some(manifest.build.clone());
    plan.prepare_fastboot()?;
    for item in manifest.partitions {
        ensure!(
            ["boot", "init_boot", "vendor_boot", "dtbo", "vbmeta"]
                .iter()
                .any(|base| item.partition == *base
                    || item.partition == format!("{base}_{}", manifest.slot)),
            "Partition is outside the restore matrix"
        );
        let file = directory.join(&item.file);
        plan.track(&file)?;
        plan.add(Invocation::fastboot(
            &d.serial,
            &["flash", &item.partition, &file.display().to_string()],
            "Restore checksum-verified partition",
        ));
    }
    plan.warnings.push("Exact device, build and slot matching is required. Transfer integrity is verified; successful boot and root restoration still need device inspection.".into());
    Ok(plan)
}
