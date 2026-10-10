use crate::{
    command::{Invocation, shell_quote},
    device::{Device, Mode},
    flash::Plan,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};
use zip::ZipArchive;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Solution {
    Magisk,
    KernelSu,
    KernelSuNext,
    SukiSu,
    WildKsu,
    APatch,
}
impl Solution {
    pub fn from_label(label: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|s| s.label() == label)
            .unwrap_or(Self::KernelSuNext)
    }
    pub const ALL: [Self; 6] = [
        Self::KernelSuNext,
        Self::KernelSu,
        Self::Magisk,
        Self::SukiSu,
        Self::WildKsu,
        Self::APatch,
    ];
    pub fn label(&self) -> &str {
        match self {
            Self::Magisk => "Magisk",
            Self::KernelSu => "KernelSU",
            Self::KernelSuNext => "KernelSU-Next",
            Self::SukiSu => "SukiSU",
            Self::WildKsu => "Wild_KSU",
            Self::APatch => "APatch",
        }
    }
    pub fn package(&self) -> &str {
        match self {
            Self::Magisk => "com.topjohnwu.magisk",
            Self::KernelSu => "me.weishu.kernelsu",
            Self::KernelSuNext => "com.rifsxd.ksunext",
            Self::SukiSu => "com.sukisu.ultra",
            Self::WildKsu => "com.twj.wksu",
            Self::APatch => "me.bmax.apatch",
        }
    }
    pub fn repo(&self) -> &str {
        match self {
            Self::Magisk => "topjohnwu/Magisk",
            Self::KernelSu => "tiann/KernelSU",
            Self::KernelSuNext => "KernelSU-Next/KernelSU-Next",
            Self::SukiSu => "SukiSU-Ultra/SukiSU-Ultra",
            Self::WildKsu => "WildKernels/Wild_KSU",
            Self::APatch => "bmax121/APatch",
        }
    }
    fn daemon(&self) -> &str {
        match self {
            Self::Magisk => "magisk",
            Self::APatch => "apd",
            _ => "ksud",
        }
    }
}
pub struct PatchJob {
    pub plan: Plan,
    pub result: PathBuf,
    pub stock: PathBuf,
    pub solution: Solution,
    pub provider_version: String,
    pub method: String,
}
pub fn patch(
    d: &Device,
    stock: &Path,
    apk: &Path,
    solution: Solution,
    kmi: &str,
    superkey: &str,
    data: &Path,
) -> Result<PatchJob> {
    patch_with_options(
        d,
        stock,
        apk,
        &PatchOptions {
            solution,
            kmi,
            superkey,
            method: crate::providers::PatchMethod::Classic,
        },
        data,
    )
}
pub struct PatchOptions<'a> {
    pub solution: Solution,
    pub kmi: &'a str,
    pub superkey: &'a str,
    pub method: crate::providers::PatchMethod,
}
pub fn patch_with_options(
    d: &Device,
    stock: &Path,
    apk: &Path,
    options: &PatchOptions<'_>,
    data: &Path,
) -> Result<PatchJob> {
    let PatchOptions {
        solution,
        kmi,
        superkey,
        method,
    } = *options;
    ensure!(
        d.mode == Mode::Adb,
        "Boot patching requires a device in Android mode"
    );
    let info = crate::image::inspect(stock)?;
    let manager = crate::providers::manager_apk(apk)?;
    ensure!(
        manager.provider == solution,
        "Manager APK belongs to a different root provider"
    );
    crate::providers::provider(solution).validate_method(&manager.version, &info.kind, method)?;
    ensure!(
        matches!(info.kind.as_str(), "boot" | "init_boot"),
        "Select boot or init_boot image"
    );
    let arch = d
        .properties
        .get("ro.product.cpu.abi")
        .context("Inspect the device to determine its CPU architecture")?;
    ensure!(
        matches!(
            arch.as_str(),
            "arm64-v8a" | "armeabi-v7a" | "x86_64" | "x86"
        ),
        "Unsupported device architecture"
    );
    if !kmi.is_empty() {
        ensure!(
            kmi.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')),
            "KMI contains invalid characters"
        );
    }
    if solution == Solution::APatch {
        ensure!(
            superkey.len() <= 1024,
            "Superkey exceeds the input size limit"
        );
        ensure!(!superkey.is_empty(), "APatch requires a superkey");
        ensure!(!superkey.contains(['\n', '\r', '\0']), "Invalid superkey");
    }
    let stamp = chrono::Utc::now().timestamp_millis();
    let local = data
        .join("patches")
        .join(format!("{stamp}-{}", &info.sha256[..8]));
    fs::create_dir_all(&local)?;
    let assets = local.join("assets");
    fs::create_dir(&assets)?;
    let mut z = ZipArchive::new(File::open(apk)?)?;
    let mut count = 0;
    for i in 0..z.len() {
        let mut entry = z.by_index(i)?;
        let safe = entry
            .enclosed_name()
            .context("Root manager APK contains unsafe paths")?;
        if entry.is_dir() {
            continue;
        }
        let path = safe.to_string_lossy();
        let name = if let Some(rest) = path.strip_prefix("assets/") {
            if rest.contains('/') {
                continue;
            }
            rest.to_owned()
        } else if let Some(rest) = path.strip_prefix(&format!("lib/{arch}/")) {
            if let Some(base) = rest.strip_prefix("lib").and_then(|v| v.strip_suffix(".so")) {
                base.to_owned()
            } else {
                continue;
            }
        } else {
            continue;
        };
        crate::command::validate_identifier(&name)?;
        ensure!(
            entry.size() < 128 * 1024 * 1024,
            "Root manager asset is too large"
        );
        let dest = assets.join(&name);
        std::io::copy(&mut entry, &mut File::create(dest)?)?;
        count += 1;
    }
    ensure!(count > 0, "APK has no compatible patching assets");
    let remote = format!("/data/local/tmp/pfr_{stamp}_{}", &info.sha256[..8]);
    let mut script = format!(
        "#!/system/bin/sh\nset -eu\ncd {}\nchmod 755 ./*\n",
        shell_quote(&remote)
    );
    script.push_str(&crate::providers::provider(solution).patch_script(
        &manager.version,
        &info.kind,
        &assets,
        kmi,
        method,
    )?);
    script.push_str("test -s patched.img\necho 'Patch image produced'\n");
    let script_path = assets.join("pfr_patch.sh");
    fs::write(&script_path, script)?;
    let result = local.join(format!(
        "{}-patched.img",
        solution.label().to_ascii_lowercase()
    ));
    let mut plan = Plan::new("Patch boot image on device", d);
    plan.destructive = true;
    plan.warnings.push(format!(
        "Provider: {} {}; method: {:?}; target: {}; hardware unverified",
        solution.label(),
        manager.version,
        method,
        info.kind
    ));
    plan.track(stock)?;
    plan.track(apk)?;
    for entry in fs::read_dir(&assets)? {
        plan.track(&entry?.path())?;
    }
    plan.add(Invocation::adb(
        &d.serial,
        &["shell", "mkdir", "-p", &remote],
        "Create isolated patch directory",
    ));
    // Push assets individually to avoid differing adb directory-copy semantics.
    for entry in fs::read_dir(&assets)? {
        let p = entry?.path();
        let target = format!("{remote}/{}", p.file_name().unwrap().to_string_lossy());
        plan.add(Invocation::adb(
            &d.serial,
            &["push", &p.display().to_string(), &target],
            "Transfer patching asset",
        ));
    }
    plan.add(Invocation::adb(
        &d.serial,
        &[
            "push",
            &stock.display().to_string(),
            &format!("{remote}/stock.img"),
        ],
        "Transfer stock image",
    ));
    let mut patch_call = Invocation::adb(
        &d.serial,
        &["shell", "sh", &format!("{remote}/pfr_patch.sh")],
        "Create patched image",
    );
    if solution == Solution::APatch {
        patch_call.needs_secret = true;
        patch_call.secret = Some(crate::command::SecretInput::new(superkey.into()));
    }
    plan.add(patch_call);
    plan.add(Invocation::adb(
        &d.serial,
        &[
            "pull",
            &format!("{remote}/patched.img"),
            &result.display().to_string(),
        ],
        "Retrieve patched image",
    ));
    plan.warnings.push("Patching writes temporary files to the selected device. It does not flash the resulting image.".into());
    Ok(PatchJob {
        plan,
        result,
        stock: stock.to_owned(),
        solution,
        provider_version: manager.version,
        method: format!("{method:?}"),
    })
}
pub fn validate_patch(stock: &Path, patched: &Path) -> Result<crate::image::ImageInfo> {
    let original = crate::image::inspect(stock)?;
    let result = crate::image::inspect(patched)?;
    ensure!(
        original.sha256 != result.sha256,
        "Patcher returned the unmodified stock image"
    );
    ensure!(
        original.kind == result.kind,
        "Patcher changed image partition type"
    );
    Ok(result)
}
#[derive(Clone, Debug)]
pub struct Module {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub disabled: bool,
    pub removed: bool,
}
pub fn parse_modules(text: &str) -> Vec<Module> {
    text.split("PFR_MODULE\n")
        .filter_map(|block| {
            let mut props = std::collections::BTreeMap::new();
            for l in block.lines() {
                if let Some((k, v)) = l.split_once('=') {
                    props.insert(k, v);
                }
            }
            let id = props.get("id")?;
            Some(Module {
                id: (*id).into(),
                name: props.get("name").copied().unwrap_or(id).into(),
                version: props.get("version").copied().unwrap_or("").into(),
                author: props.get("author").copied().unwrap_or("").into(),
                description: props.get("description").copied().unwrap_or("").into(),
                disabled: props.get("pfr_disabled") == Some(&"1"),
                removed: props.get("pfr_removed") == Some(&"1"),
            })
        })
        .collect()
}
pub fn module_list(d: &Device) -> Invocation {
    Invocation::adb(
        &d.serial,
        &[
            "shell",
            "su",
            "-c",
            "for p in /data/adb/modules/*; do [ -f \"$p/module.prop\" ] || continue; echo PFR_MODULE; cat \"$p/module.prop\"; [ ! -f \"$p/disable\" ] || echo pfr_disabled=1; [ ! -f \"$p/remove\" ] || echo pfr_removed=1; done",
        ],
        "Read root modules",
    )
}
pub fn module_action(d: &Device, id: &str, action: &str) -> Result<Plan> {
    crate::command::validate_identifier(id)?;
    let path = format!("/data/adb/modules/{id}");
    let command = match action {
        "disable" => format!("touch {path}/disable"),
        "enable" => format!("rm -f {path}/disable"),
        "remove" => format!("touch {path}/remove"),
        "restore" => format!("rm -f {path}/remove"),
        "action" => format!("cd {path} && sh action.sh"),
        _ => anyhow::bail!("Unknown module action"),
    };
    let mut p = Plan::new(&format!("{action} module {id}"), d);
    p.destructive = true;
    p.add(Invocation::adb(
        &d.serial,
        &["shell", "su", "-c", &command],
        "Modify module state",
    ));
    Ok(p)
}
pub fn install_module(d: &Device, path: &Path, solution: Solution) -> Result<Plan> {
    ensure!(
        d.mode == Mode::Adb && d.rooted,
        "Module installation requires an inspected rooted Android device"
    );
    let remote = "/data/local/tmp/pfr_module.zip";
    let mut p = Plan::new("Install root module", d);
    p.destructive = true;
    p.track(path)?;
    p.add(Invocation::adb(
        &d.serial,
        &["push", &path.display().to_string(), remote],
        "Transfer module",
    ));
    let daemon = solution.daemon();
    let cmd = if solution == Solution::Magisk {
        format!("magisk --install-module {remote}")
    } else {
        format!("/data/adb/{daemon} module install {remote}")
    };
    p.add(Invocation::adb(
        &d.serial,
        &["shell", "su", "-c", &cmd],
        "Install module",
    ));
    Ok(p)
}
