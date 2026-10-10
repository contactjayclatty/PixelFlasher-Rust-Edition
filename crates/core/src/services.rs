use crate::{
    command::{Invocation, Tool, Transport, quiet, shell_quote, validate_identifier},
    device::{Device, Mode},
    flash::Plan,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

#[derive(Clone, Debug, Default)]
pub struct Package {
    pub name: String,
    pub path: String,
    pub uid: String,
    pub disabled: bool,
    pub system: bool,
}
pub fn parse_packages(text: &str) -> Vec<Package> {
    text.lines()
        .filter_map(|l| {
            let l = l.strip_prefix("package:")?;
            let (name, uid) = l.split_once(" uid:").unwrap_or((l, ""));
            let (path, name) = name.rsplit_once('=').unwrap_or(("", name));
            Some(Package {
                name: name.into(),
                path: path.into(),
                uid: uid.into(),
                ..Default::default()
            })
        })
        .collect()
}
pub fn packages(t: &dyn Transport, d: &Device) -> Result<Vec<Package>> {
    let mut items = parse_packages(
        &quiet(
            t,
            &Invocation::adb(
                &d.serial,
                &["shell", "pm", "list", "packages", "-f", "-U"],
                "Read installed apps",
            ),
        )?
        .stdout,
    );
    let disabled = parse_packages(
        &quiet(
            t,
            &Invocation::adb(
                &d.serial,
                &["shell", "pm", "list", "packages", "-d"],
                "Read disabled apps",
            ),
        )?
        .stdout,
    );
    let system = parse_packages(
        &quiet(
            t,
            &Invocation::adb(
                &d.serial,
                &["shell", "pm", "list", "packages", "-s"],
                "Read system apps",
            ),
        )?
        .stdout,
    );
    for p in &mut items {
        p.disabled = disabled.iter().any(|v| v.name == p.name);
        p.system = system.iter().any(|v| v.name == p.name);
    }
    items.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(items)
}
pub fn package_action(d: &Device, packages: &[String], action: &str) -> Result<Plan> {
    ensure!(d.mode == Mode::Adb, "App management requires Android mode");
    let mut p = Plan::new(&format!("{action} selected applications"), d);
    p.destructive = !matches!(action, "details" | "permissions" | "launch");
    for name in packages {
        validate_identifier(name)?;
        let args = match action {
            "enable" => vec!["shell", "pm", "enable", name],
            "disable" => vec!["shell", "pm", "disable-user", "--user", "0", name],
            "uninstall" => vec!["shell", "pm", "uninstall", "--user", "0", name],
            "clear" => vec!["shell", "pm", "clear", name],
            "kill" => vec!["shell", "am", "force-stop", name],
            "details" | "permissions" => vec!["shell", "dumpsys", "package", name],
            "launch" => vec![
                "shell",
                "monkey",
                "-p",
                name,
                "-c",
                "android.intent.category.LAUNCHER",
                "1",
            ],
            _ => anyhow::bail!("Unknown app action"),
        };
        p.add(Invocation::adb(&d.serial, &args, "Application action"));
    }
    ensure!(!p.steps.is_empty(), "Select at least one application");
    Ok(p)
}
pub fn install_apk(d: &Device, path: &Path, store_owner: bool, bypass: bool) -> Result<Plan> {
    let mut p = Plan::new("Install Android application", d);
    p.destructive = true;
    p.track(path)?;
    let mut args = vec!["install", "-r"];
    if store_owner {
        args.extend(["-i", "com.android.vending"]);
    }
    if bypass {
        args.push("--bypass-low-target-sdk-block");
    }
    let file = path.display().to_string();
    args.push(&file);
    p.add(Invocation::adb(&d.serial, &args, "Install APK"));
    Ok(p)
}
pub fn pull_apk(
    t: &dyn Transport,
    d: &Device,
    package: &str,
    directory: &Path,
    cancel: &AtomicBool,
    log: &(dyn Fn(String) + Send + Sync),
) -> Result<Vec<PathBuf>> {
    validate_identifier(package)?;
    fs::create_dir_all(directory)?;
    let out = quiet(
        t,
        &Invocation::adb(
            &d.serial,
            &["shell", "pm", "path", package],
            "Locate package APKs",
        ),
    )?;
    let mut paths = vec![];
    for (i, line) in out.stdout.lines().enumerate() {
        let Some(remote) = line.strip_prefix("package:") else {
            continue;
        };
        let name = Path::new(remote)
            .file_name()
            .context("APK path has no filename")?
            .to_string_lossy();
        let local = directory.join(format!("{package}-{i}-{name}"));
        ensure!(!local.exists(), "Destination APK exists");
        t.execute(
            &Invocation::adb(
                &d.serial,
                &["pull", remote, &local.display().to_string()],
                "Download installed APK",
            ),
            cancel,
            log,
        )?
        .require_success()?;
        paths.push(local);
    }
    ensure!(!paths.is_empty(), "No APK path returned");
    Ok(paths)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Download {
    pub name: String,
    pub url: String,
    pub sha256: Option<String>,
    pub size: Option<u64>,
}
pub fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .https_only(true)
        .user_agent(format!(
            "PixelFlasher-Rust/{} (source port)",
            env!("CARGO_PKG_VERSION")
        ))
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(3600))
        .build()?)
}
pub fn releases(repo: &str) -> Result<Vec<Download>> {
    ensure!(
        repo.split('/').count() == 2
            && repo
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '.')),
        "Invalid GitHub repository"
    );
    let data: serde_json::Value = client()?
        .get(format!("https://api.github.com/repos/{repo}/releases"))
        .send()?
        .error_for_status()?
        .json()?;
    let mut result = vec![];
    for release in data.as_array().context("Unexpected release response")? {
        for asset in release["assets"].as_array().into_iter().flatten() {
            if let (Some(name), Some(url)) = (
                asset["name"].as_str(),
                asset["browser_download_url"].as_str(),
            ) {
                result.push(Download {
                    name: format!(
                        "{} / {name}",
                        release["tag_name"].as_str().unwrap_or("release")
                    ),
                    url: url.into(),
                    sha256: asset["digest"]
                        .as_str()
                        .and_then(|s| s.strip_prefix("sha256:"))
                        .map(str::to_owned),
                    size: asset["size"].as_u64(),
                });
            }
        }
    }
    Ok(result)
}
pub fn download(
    item: &Download,
    path: &Path,
    cancel: &AtomicBool,
    log: &dyn Fn(String),
) -> Result<()> {
    ensure!(item.url.starts_with("https://"), "Downloads require HTTPS");
    ensure!(!path.exists(), "Destination exists; choose a new filename");
    let tmp = path.with_extension("partial");
    let metadata = path.with_extension("resume.json");
    let mut request = client()?
        .get(&item.url)
        .header(reqwest::header::ACCEPT_ENCODING, "identity");
    let offset = if tmp.exists() {
        fs::metadata(&tmp)?.len()
    } else {
        0
    };
    let previous: Option<ResumeMetadata> = if offset > 0 {
        Some(serde_json::from_slice(&fs::read(&metadata).context(
            "Partial download has no resume metadata; choose a new destination",
        )?)?)
    } else {
        None
    };
    if let Some(old) = &previous {
        ensure!(
            old.url == item.url && old.sha256 == item.sha256 && old.size == item.size,
            "Download source or checksum changed; do not combine partial data"
        );
        ensure!(
            !old.etag.is_empty() && !old.etag.starts_with("W/"),
            "Server did not supply a strong resume validator; choose a new destination"
        );
        request = request
            .header(reqwest::header::RANGE, format!("bytes={offset}-"))
            .header(reqwest::header::IF_RANGE, &old.etag);
    }
    let mut response = request.send()?.error_for_status()?;
    let etag = response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let total = if offset > 0 {
        ensure!(
            response.status() == reqwest::StatusCode::PARTIAL_CONTENT,
            "Server refused resume; partial data retained"
        );
        ensure!(
            previous.as_ref().is_some_and(|p| p.etag == etag),
            "Remote file validator changed"
        );
        Some(validate_resume_range(
            response
                .headers()
                .get(reqwest::header::CONTENT_RANGE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or(""),
            offset,
        )?)
    } else {
        response.content_length()
    };
    fs::write(
        &metadata,
        serde_json::to_vec(&ResumeMetadata {
            url: item.url.clone(),
            sha256: item.sha256.clone(),
            size: item.size,
            etag,
        })?,
    )?;
    let mut out = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&tmp)?;
    let mut buf = [0; 128 * 1024];
    let mut bytes = offset;
    let mut last = 0;
    loop {
        ensure!(
            !cancel.load(Ordering::Relaxed),
            "Download cancelled; partial file retained"
        );
        let n = response.read(&mut buf)?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])?;
        bytes += n as u64;
        if bytes / 1048576 > last {
            last = bytes / 1048576;
            log(format!(
                "Downloaded {} MiB{}",
                last,
                total
                    .map(|v| format!(" / {} MiB", v / 1048576))
                    .unwrap_or_default()
            ));
        }
    }
    out.sync_all()?;
    drop(out);
    if let Some(size) = item.size.or(total) {
        ensure!(bytes == size, "Downloaded size mismatch; partial retained");
    }
    if let Some(expected) = &item.sha256 {
        ensure!(
            crate::image::hashes(&tmp)?.1.eq_ignore_ascii_case(expected),
            "Download SHA-256 mismatch; partial retained"
        );
    }
    fs::rename(tmp, path)?;
    fs::remove_file(metadata)?;
    log(format!("Saved {}", path.display()));
    Ok(())
}
#[derive(Serialize, Deserialize)]
struct ResumeMetadata {
    url: String,
    sha256: Option<String>,
    size: Option<u64>,
    etag: String,
}
pub fn validate_resume_range(header: &str, offset: u64) -> Result<u64> {
    let range = header
        .strip_prefix("bytes ")
        .context("Missing resume range")?;
    let (span, total) = range.split_once('/').context("Invalid resume range")?;
    let (start, end) = span.split_once('-').context("Invalid resume span")?;
    let start: u64 = start.parse()?;
    let end: u64 = end.parse()?;
    let total: u64 = total.parse()?;
    ensure!(
        start == offset && end >= start && end < total,
        "Resume offset or length does not match"
    );
    Ok(total)
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogName {
    pub model: String,
    pub build: String,
}
pub fn catalog_name(file_name: &str) -> Option<CatalogName> {
    let stem = file_name.strip_suffix(".zip")?;
    let (model, rest) = stem.split_once('-')?;
    if model.is_empty()
        || !model
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    {
        return None;
    }
    let build = if let Some(after) = rest.strip_prefix("ota-") {
        let (build, hash) = after.rsplit_once('-')?;
        if build.is_empty() || !is_short_hash(hash) {
            return None;
        }
        build
    } else {
        let (build, hash) = rest.rsplit_once("-factory-")?;
        if build.is_empty() || !is_short_hash(hash) {
            return None;
        }
        build
    };
    Some(CatalogName {
        model: model.to_owned(),
        build: build.to_owned(),
    })
}
pub fn catalog_model(file_name: &str) -> String {
    catalog_name(file_name)
        .map(|name| name.model)
        .unwrap_or_else(|| "Other".into())
}
pub fn catalog_build(file_name: &str) -> String {
    catalog_name(file_name)
        .map(|name| name.build)
        .unwrap_or_else(|| "—".into())
}
fn is_short_hash(value: &str) -> bool {
    value.len() >= 8 && value.chars().all(|c| c.is_ascii_hexdigit())
}
pub fn parse_google_images(html: &str) -> Vec<Download> {
    let re = regex::Regex::new(
        r#"https://(?:dl\.google\.com|storage\.googleapis\.com)/[^\s\"<>]+\.zip"#,
    )
    .unwrap();
    let mut items = vec![];
    let hash_pattern = regex::Regex::new(r"\b[a-fA-F0-9]{64}\b").unwrap();
    for found in re.find_iter(html) {
        let url = found.as_str();
        if items.iter().any(|d: &Download| d.url == url) {
            continue;
        }
        // Never associate another row/download's checksum with this image.
        let tail = &html[found.end()..];
        let boundary = tail
            .find("</tr>")
            .unwrap_or(tail.len())
            .min(re.find(tail).map(|m| m.start()).unwrap_or(tail.len()))
            .min(1200);
        let after = &tail[..boundary];
        let hash = hash_pattern
            .find(after)
            .map(|m| m.as_str().to_ascii_lowercase());
        items.push(Download {
            name: url.rsplit('/').next().unwrap_or("firmware.zip").into(),
            url: url.into(),
            sha256: hash,
            size: None,
        });
    }
    items
}
pub fn google_catalog_cookie(ota: bool, terms_accepted: bool) -> Result<&'static str> {
    ensure!(
        terms_accepted,
        "Read and acknowledge Google's firmware terms before loading the catalog"
    );
    Ok(if ota {
        "devsite_wall_acks=nexus-ota-tos"
    } else {
        "devsite_wall_acks=nexus-image-tos"
    })
}
pub fn firmware_catalog(ota: bool, terms_accepted: bool) -> Result<Vec<Download>> {
    let cookie = google_catalog_cookie(ota, terms_accepted)?;
    let url = if ota {
        "https://developers.google.com/android/ota"
    } else {
        "https://developers.google.com/android/images"
    };
    let response = client()?
        .get(url)
        .header(reqwest::header::COOKIE, cookie)
        .timeout(Duration::from_secs(45))
        .send()?
        .error_for_status()?;
    ensure!(
        response.url().host_str() == Some("developers.google.com"),
        "Google catalog redirected to an unexpected host"
    );
    let mut html = String::new();
    response
        .take(8 * 1024 * 1024 + 1)
        .read_to_string(&mut html)?;
    ensure!(
        html.len() <= 8 * 1024 * 1024,
        "Google catalog exceeds the supported page size"
    );
    let items = parse_google_images(&html);
    ensure!(
        !items.is_empty(),
        "Google did not return its firmware download table. Confirm the terms acknowledgement, retry, or open the official catalog in your browser"
    );
    Ok(items)
}
pub fn wireless(action: &str, endpoint: &str, code: &str) -> Result<Invocation> {
    ensure!(
        endpoint.len() < 256
            && !endpoint.is_empty()
            && endpoint
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '[' | ']' | '-')),
        "Enter an IP address/hostname and port"
    );
    let args = match action {
        "pair" => {
            ensure!(
                code.len() == 6 && code.chars().all(|c| c.is_ascii_digit()),
                "Pairing code must be six digits"
            );
            vec!["pair", endpoint, code]
        }
        "connect" => vec!["connect", endpoint],
        "disconnect" => vec!["disconnect", endpoint],
        _ => anyhow::bail!("Unknown wireless action"),
    };
    Ok(Invocation::global(Tool::Adb, &args, "Wireless ADB"))
}
pub fn partitions(t: &dyn Transport, d: &Device) -> Result<Vec<String>> {
    let call = if d.mode == Mode::Fastboot {
        Invocation::fastboot(&d.serial, &["getvar", "all"], "Read partition variables")
    } else {
        Invocation::adb(
            &d.serial,
            &["shell", "su", "-c", "ls -1 /dev/block/by-name"],
            "List partitions",
        )
    };
    let text = quiet(t, &call)?.text();
    let mut result = if d.mode == Mode::Fastboot {
        crate::device::parse_variables(&text)
            .keys()
            .filter_map(|k| k.strip_prefix("partition-size:").map(str::to_owned))
            .collect::<Vec<_>>()
    } else {
        text.lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect()
    };
    result.sort();
    result.dedup();
    Ok(result)
}
pub fn backup_partition(d: &Device, name: &str, dest: &Path) -> Result<Plan> {
    validate_identifier(name)?;
    ensure!(
        d.mode == Mode::Adb && d.rooted,
        "Partition backup requires root in Android mode"
    );
    ensure!(!dest.exists(), "Backup destination already exists");
    let remote = format!("/data/local/tmp/pfr_{}.img", name);
    let command =
        format!("dd if=/dev/block/by-name/{name} of={remote} bs=1048576 && chmod 644 {remote}");
    let mut p = Plan::new("Back up partition", d);
    p.add(Invocation::adb(
        &d.serial,
        &["shell", "su", "-c", &command],
        "Read partition into temporary backup",
    ));
    p.add(Invocation::adb(
        &d.serial,
        &["pull", &remote, &dest.display().to_string()],
        "Retrieve partition backup",
    ));
    Ok(p)
}
pub fn data_backup(d: &Device, dest: &Path, restore: bool) -> Result<Plan> {
    ensure!(
        d.mode == Mode::Adb && d.rooted,
        "Root data backup requires root in Android mode"
    );
    let mut p = Plan::new(
        if restore {
            "Restore root data"
        } else {
            "Back up root data"
        },
        d,
    );
    p.destructive = restore;
    let remote = "/data/local/tmp/pfr_data_adb.tar.gz";
    if restore {
        crate::advanced::validate_root_archive(dest)?;
        p.track(dest)?;
        p.add(Invocation::adb(
            &d.serial,
            &["push", &dest.display().to_string(), remote],
            "Transfer backup",
        ));
        p.add(Invocation::adb(
            &d.serial,
            &[
                "shell",
                "su",
                "-c",
                "tar -xzpf /data/local/tmp/pfr_data_adb.tar.gz -C /data",
            ],
            "Restore /data/adb",
        ));
    } else {
        ensure!(!dest.exists(), "Destination exists");
        p.add(Invocation::adb(&d.serial,&["shell","su","-c","tar -czpf /data/local/tmp/pfr_data_adb.tar.gz -C /data adb && chmod 644 /data/local/tmp/pfr_data_adb.tar.gz"],"Archive /data/adb"));
        p.add(Invocation::adb(
            &d.serial,
            &["pull", remote, &dest.display().to_string()],
            "Retrieve root data archive",
        ));
    }
    Ok(p)
}
pub fn rooted_file_read(t: &dyn Transport, d: &Device, path: &str) -> Result<String> {
    ensure!(
        path.starts_with('/') && !path.contains(['\n', '\r', '\0']),
        "Invalid device path"
    );
    Ok(quiet(
        t,
        &Invocation::adb(
            &d.serial,
            &["shell", "su", "-c", &format!("cat {}", shell_quote(path))],
            "Read device configuration",
        ),
    )?
    .stdout)
}
pub fn rooted_file_write(d: &Device, path: &str, content: &str, data: &Path) -> Result<Plan> {
    ensure!(
        path.starts_with("/data/adb/") && !path.contains(['\n', '\r', '\0']),
        "Root configuration editor is restricted to /data/adb"
    );
    ensure!(
        !path
            .split('/')
            .any(|component| component == ".." || component == "."),
        "Configuration path contains traversal"
    );
    let local = data.join(format!(
        "edit-{}.txt",
        chrono::Utc::now().timestamp_millis()
    ));
    let normalized = if path.ends_with(".json") {
        Some(crate::config::normalize_json(content)?)
    } else {
        None
    };
    fs::write(&local, normalized.as_deref().unwrap_or(content))?;
    let remote = "/data/local/tmp/pfr_config.txt";
    let mut p = Plan::new("Save root configuration", d);
    p.destructive = true;
    p.track(&local)?;
    p.add(Invocation::adb(
        &d.serial,
        &["push", &local.display().to_string(), remote],
        "Transfer edited configuration",
    ));
    let cmd = format!(
        "cp {} {} && chmod 644 {}",
        shell_quote(remote),
        shell_quote(path),
        shell_quote(path)
    );
    p.add(Invocation::adb(
        &d.serial,
        &["shell", "su", "-c", &cmd],
        "Save configuration",
    ));
    Ok(p)
}
