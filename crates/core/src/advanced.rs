use crate::{
    command::{Invocation, Transport, quiet, shell_quote, validate_identifier},
    device::{Device, Mode},
    flash::Plan,
};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};
use x509_parser::prelude::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CertificateReport {
    pub subject: String,
    pub issuer: String,
    pub serial: String,
    pub not_before: String,
    pub not_after: String,
    pub currently_valid: bool,
    pub chain_signature_valid: Option<bool>,
    pub revocation: String,
}
pub fn keybox_certificates(
    xml: &str,
    revocations: Option<&serde_json::Value>,
) -> Result<Vec<CertificateReport>> {
    let re = regex::Regex::new(r"(?s)<Certificate[^>]*>(.*?)</Certificate>")?;
    let mut certs = vec![];
    for cap in re.captures_iter(xml) {
        let raw = cap[1].trim();
        let body = raw
            .replace("-----BEGIN CERTIFICATE-----", "")
            .replace("-----END CERTIFICATE-----", "");
        let body: String = body.chars().filter(|c| !c.is_whitespace()).collect();
        certs.push(
            STANDARD
                .decode(body)
                .context("Invalid certificate base64")?,
        );
    }
    ensure!(!certs.is_empty(), "No Certificate elements found");
    let mut reports = vec![];
    for (i, der) in certs.iter().enumerate() {
        let (_, cert) = parse_x509_certificate(der)
            .map_err(|e| anyhow::anyhow!("Invalid X509 certificate: {e}"))?;
        let serial = cert
            .raw_serial_as_string()
            .replace(':', "")
            .trim_start_matches('0')
            .to_ascii_lowercase();
        let signature = if let Some(parent) = certs.get(i + 1) {
            let (_, parent) = parse_x509_certificate(parent)
                .map_err(|e| anyhow::anyhow!("Invalid parent certificate: {e}"))?;
            if parent.subject() == cert.issuer() {
                Some(cert.verify_signature(Some(parent.public_key())).is_ok())
            } else {
                None
            }
        } else if cert.subject() == cert.issuer() {
            Some(cert.verify_signature(None).is_ok())
        } else {
            None
        };
        let revocation = if let Some(data) = revocations {
            let matched = data["entries"]
                .as_object()
                .and_then(|entries| {
                    entries
                        .iter()
                        .find(|(key, _)| key.trim_start_matches('0').eq_ignore_ascii_case(&serial))
                })
                .map(|(_, v)| v);
            matched
                .map(|v| {
                    format!(
                        "{}: {}",
                        v["status"].as_str().unwrap_or("listed"),
                        v["reason"].as_str().unwrap_or("unspecified")
                    )
                })
                .unwrap_or_else(|| "Not listed in fetched status data".into())
        } else {
            "Not checked".into()
        };
        reports.push(CertificateReport {
            subject: cert.subject().to_string(),
            issuer: cert.issuer().to_string(),
            serial,
            not_before: cert.validity().not_before.to_string(),
            not_after: cert.validity().not_after.to_string(),
            currently_valid: cert.validity().is_valid(),
            chain_signature_valid: signature,
            revocation,
        });
    }
    Ok(reports)
}
pub fn keybox_report(path: &Path, online: bool) -> Result<Vec<CertificateReport>> {
    let text = fs::read_to_string(path)?;
    let statuses = if online {
        Some(
            crate::services::client()?
                .get("https://android.googleapis.com/attestation/status")
                .send()?
                .error_for_status()?
                .json()?,
        )
    } else {
        None
    };
    keybox_certificates(&text, statuses.as_ref())
}
pub fn apk_manifest(path: &Path) -> Result<String> {
    let mut zip = zip::ZipArchive::new(File::open(path)?)?;
    let mut entry = zip.by_name("AndroidManifest.xml")?;
    ensure!(
        entry.size() <= 32 * 1024 * 1024,
        "APK manifest is too large"
    );
    let mut data = vec![];
    entry.read_to_end(&mut data)?;
    crate::xml::decode(&data)
}
pub fn magisk_permission(
    d: &Device,
    uid: &str,
    policy: i32,
    minutes: u64,
    notify: bool,
    logging: bool,
) -> Result<Plan> {
    ensure!(
        d.mode == Mode::Adb && d.rooted,
        "Superuser policy changes require rooted Android mode"
    );
    ensure!(
        !uid.is_empty() && uid.chars().all(|c| c.is_ascii_digit()),
        "UID must be numeric"
    );
    ensure!(matches!(policy, 0..=2), "Invalid SU policy");
    ensure!(minutes <= 525_600, "Policy duration exceeds one year");
    let until = if minutes == 0 {
        0
    } else {
        chrono::Utc::now().timestamp() + minutes as i64 * 60
    };
    let sql = if policy == 0 {
        format!("DELETE FROM policies WHERE uid={uid};")
    } else {
        format!(
            "INSERT OR REPLACE INTO policies(uid,policy,until,logging,notification) VALUES({uid},{policy},{until},{},{});",
            i32::from(logging),
            i32::from(notify)
        )
    };
    let mut plan = Plan::new("Change Magisk SU policy", d);
    plan.destructive = true;
    plan.add(Invocation::adb(
        &d.serial,
        &[
            "shell",
            "su",
            "-c",
            &format!("magisk --sqlite {}", shell_quote(&sql)),
        ],
        "Update superuser policy",
    ));
    Ok(plan)
}
pub fn denylist(d: &Device, package: &str, add: bool) -> Result<Plan> {
    validate_identifier(package)?;
    let command = format!(
        "magisk --denylist {} {}",
        if add { "add" } else { "rm" },
        shell_quote(package)
    );
    let mut p = Plan::new("Change Magisk denylist", d);
    p.destructive = true;
    p.add(Invocation::adb(
        &d.serial,
        &["shell", "su", "-c", &command],
        "Modify denylist",
    ));
    Ok(p)
}
pub fn magisk_backups(t: &dyn Transport, d: &Device) -> Result<Vec<String>> {
    let text = quiet(
        t,
        &Invocation::adb(
            &d.serial,
            &[
                "shell",
                "su",
                "-c",
                "for p in /data/magisk_backup_*; do [ ! -d \"$p\" ] || basename \"$p\"; done",
            ],
            "List Magisk backups",
        ),
    )?
    .stdout;
    Ok(text
        .lines()
        .filter_map(|l| l.strip_prefix("magisk_backup_"))
        .filter(|h| h.len() == 40 && h.chars().all(|c| c.is_ascii_hexdigit()))
        .map(str::to_owned)
        .collect())
}
pub fn delete_magisk_backup(d: &Device, sha1: &str) -> Result<Plan> {
    ensure!(
        sha1.len() == 40 && sha1.chars().all(|c| c.is_ascii_hexdigit()),
        "Backup must be identified by a SHA-1 hash"
    );
    let mut p = Plan::new("Delete Magisk backup", d);
    p.destructive = true;
    p.add(Invocation::adb(
        &d.serial,
        &[
            "shell",
            "su",
            "-c",
            &format!("rm -rf /data/magisk_backup_{sha1}"),
        ],
        "Delete selected backup",
    ));
    Ok(p)
}
pub fn add_magisk_backup(d: &Device, image: &Path, data: &Path) -> Result<Plan> {
    let info = crate::image::inspect(image)?;
    ensure!(
        matches!(info.kind.as_str(), "boot" | "init_boot"),
        "Select a boot image"
    );
    let local = data.join(format!("stock-{}.img.gz", info.sha1));
    let mut encoder =
        flate2::write::GzEncoder::new(File::create(&local)?, flate2::Compression::default());
    std::io::copy(&mut File::open(image)?, &mut encoder)?;
    encoder.finish()?;
    let remote = "/data/local/tmp/pfr_stock_boot.img.gz";
    let mut p = Plan::new("Add Magisk stock backup", d);
    p.destructive = true;
    p.track(&local)?;
    p.add(Invocation::adb(
        &d.serial,
        &["push", &local.display().to_string(), remote],
        "Transfer compressed stock image",
    ));
    p.add(Invocation::adb(
        &d.serial,
        &[
            "shell",
            "su",
            "-c",
            &format!(
                "mkdir -p /data/magisk_backup_{} && cp {remote} /data/magisk_backup_{}/boot.img.gz",
                info.sha1, info.sha1
            ),
        ],
        "Create Magisk backup",
    ));
    Ok(p)
}
pub fn validate_root_archive(path: &Path) -> Result<()> {
    let input = flate2::read::GzDecoder::new(File::open(path)?);
    let mut tar = tar::Archive::new(input);
    let mut found = false;
    for entry in tar.entries()? {
        let entry = entry?;
        let path = entry.path()?;
        ensure!(
            path.components().all(|c| matches!(
                c,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )),
            "Archive contains path traversal"
        );
        let first = path
            .components()
            .find(|c| matches!(c, std::path::Component::Normal(_)));
        ensure!(
            first.is_some_and(|c| c.as_os_str() == "adb"),
            "Archive contains content outside /data/adb"
        );
        if let Some(target) = entry.link_name()? {
            ensure!(
                !target.is_absolute()
                    && !target
                        .components()
                        .any(|c| matches!(c, std::path::Component::ParentDir)),
                "Archive contains an escaping link"
            );
        }
        found = true;
    }
    ensure!(found, "Archive is empty");
    Ok(())
}
pub fn integrity_report(t: &dyn Transport, d: &Device) -> Result<String> {
    let mut sections: BTreeMap<String, String> = BTreeMap::new();
    let props: BTreeMap<_, _> = d
        .properties
        .iter()
        .filter(|(k, _)| {
            !k.to_lowercase().contains("serial")
                && !k.to_lowercase().contains("imei")
                && !k.to_lowercase().contains("imsi")
        })
        .collect();
    sections.insert(
        "device_state".into(),
        serde_json::to_value(props)?.to_string(),
    );
    for (name, cmd) in [
        (
            "root_modules",
            "for p in /data/adb/modules/*; do [ ! -f \"$p/module.prop\" ] || cat \"$p/module.prop\"; done",
        ),
        ("magisk_denylist", "magisk --denylist ls"),
        (
            "tricky_store_status",
            "for p in /data/adb/tricky_store/target.txt /data/adb/tricky_store/security_patch.txt /data/adb/tricky_store/tee_status; do [ ! -f \"$p\" ] || { echo \"$p\"; cat \"$p\"; }; done",
        ),
        (
            "integrity_modules",
            "ls -la /data/adb/modules/playintegrityfix /data/adb/modules/targetedfix",
        ),
    ] {
        let value = quiet(
            t,
            &Invocation::adb(
                &d.serial,
                &["shell", "su", "-c", cmd],
                "Read integrity diagnostics",
            ),
        )
        .map(|o| o.stdout)
        .unwrap_or_else(|e| format!("Unavailable: {e}"));
        sections.insert(name.into(), value);
    }
    let logs = quiet(
        t,
        &Invocation::adb(
            &d.serial,
            &[
                "logcat",
                "-d",
                "-t",
                "500",
                "-s",
                "PlayIntegrity",
                "TrickyStore",
            ],
            "Read integrity logs",
        ),
    )
    .map(|o| o.stdout)
    .unwrap_or_default();
    sections.insert("recent_logs".into(), logs);
    let mut text = serde_json::to_string_pretty(&sections)?;
    text = text.replace(&d.serial, "[device]");
    Ok(text)
}
pub fn support_zip(path: &Path, log: &[String], settings: &crate::config::Config) -> Result<()> {
    ensure!(!path.exists(), "Support ZIP already exists");
    let mut zip = zip::ZipWriter::new(File::create(path)?);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut safe = settings.clone();
    safe.tools.clear();
    for d in &mut safe.devices {
        d.serial = "[device]".into();
    }
    zip.start_file("settings-redacted.json", options)?;
    zip.write_all(&serde_json::to_vec_pretty(&safe)?)?;
    zip.start_file("session.log", options)?;
    let mut text = log.join("\n");
    for d in &settings.devices {
        text = text.replace(&d.serial, "[device]");
    }
    zip.write_all(text.as_bytes())?;
    zip.start_file("version.txt", options)?;
    zip.write_all(
        format!(
            "PixelFlasher Rust Edition {}\nUpstream 081286d6baf8ac969df29e8b0ab3a5b6ae4cfb54\n",
            env!("CARGO_PKG_VERSION")
        )
        .as_bytes(),
    )?;
    zip.finish()?;
    Ok(())
}
