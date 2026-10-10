use crate::{
    command::{Invocation, Tool, Transport, quiet},
    device::{Device, Mode, parse_variables},
    firmware::Firmware,
    image,
};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum Slot {
    #[default]
    Current,
    Inactive,
    Both,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FlashOptions {
    pub wipe: bool,
    pub slot: Slot,
    pub disable_verity: bool,
    pub disable_verification: bool,
    pub force: bool,
    pub no_reboot: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Artifact {
    pub path: PathBuf,
    pub sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Step {
    Command(Invocation),
    WaitFastboot,
    Sleep(u64),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    pub title: String,
    pub device: Device,
    pub steps: Vec<Step>,
    pub artifacts: Vec<Artifact>,
    pub destructive: bool,
    pub warnings: Vec<String>,
    pub requires_unlocked: bool,
    #[serde(default)]
    pub expected_uids: Vec<(String, String)>,
    #[serde(default)]
    pub expected_build: Option<String>,
}
impl Plan {
    pub fn new(title: &str, device: &Device) -> Self {
        Self {
            title: title.into(),
            device: device.clone(),
            steps: vec![],
            artifacts: vec![],
            destructive: false,
            warnings: vec![],
            requires_unlocked: false,
            expected_uids: vec![],
            expected_build: None,
        }
    }
    pub fn add(&mut self, mut call: Invocation) {
        // Typed planner ownership protects every privileged mutation, including
        // provider commands whose flags differ between versions.
        if self.destructive && call.tool == Tool::Adb && call.args.iter().any(|v| v == "su") {
            call.critical = true;
        }
        self.steps.push(Step::Command(call));
    }
    pub fn track(&mut self, path: &Path) -> Result<()> {
        ensure!(path.is_file(), "Input file missing: {}", path.display());
        let (_, sha256) = image::hashes(path)?;
        self.artifacts.push(Artifact {
            path: path.to_owned(),
            sha256,
        });
        Ok(())
    }
    pub fn prepare_fastboot(&mut self) -> Result<()> {
        ensure!(!self.device.serial.is_empty(), "Select a device");
        match self.device.mode {
            Mode::Fastboot => {}
            Mode::Adb | Mode::Recovery => {
                self.add(Invocation::adb(
                    &self.device.serial,
                    &["reboot", "bootloader"],
                    "Reboot to bootloader",
                ));
                self.steps.push(Step::WaitFastboot);
            }
            _ => bail!("Device must be in Android, recovery or fastboot mode"),
        };
        Ok(())
    }
    pub fn validate_confirmation(&self, confirmation: &str) -> Result<()> {
        ensure!(
            confirmation.trim() == self.device.confirmation(),
            "Type the exact device confirmation phrase"
        );
        Ok(())
    }
}
fn slots(d: &Device, slot: &Slot) -> Result<Vec<Option<String>>> {
    Ok(match slot {
        Slot::Current => vec![None],
        Slot::Both => {
            ensure!(
                matches!(d.slot.as_str(), "a" | "b"),
                "A/B slot state must be known"
            );
            vec![Some("a".into()), Some("b".into())]
        }
        Slot::Inactive => {
            ensure!(
                matches!(d.slot.as_str(), "a" | "b"),
                "Active slot is unknown"
            );
            vec![Some(if d.slot == "a" { "b" } else { "a" }.into())]
        }
    })
}
fn args_with_options(options: &FlashOptions) -> Vec<String> {
    let mut a = vec![];
    if options.disable_verity {
        a.push("--disable-verity".into());
    }
    if options.disable_verification {
        a.push("--disable-verification".into());
    }
    if options.force {
        a.push("--force".into());
    }
    a
}
fn fastboot(d: &Device, args: Vec<String>, label: &str) -> Invocation {
    let refs: Vec<_> = args.iter().map(String::as_str).collect();
    let mut call = Invocation::fastboot(&d.serial, &refs, label);
    call.timeout = 900;
    call
}
pub fn boot_image(
    d: &Device,
    path: &Path,
    partition: &str,
    live: bool,
    options: &FlashOptions,
) -> Result<Plan> {
    crate::command::validate_identifier(partition)?;
    let info = image::inspect(path)?;
    if matches!(partition, "boot" | "init_boot" | "vendor_boot") {
        ensure!(
            info.kind == partition,
            "Selected image is {}, but target is {partition}",
            info.kind
        );
    }
    ensure!(
        !live || partition == "boot",
        "Only boot images can be live-booted"
    );
    let mut p = Plan::new(
        if live {
            "Live boot image"
        } else {
            "Flash selected image"
        },
        d,
    );
    p.destructive = !live;
    p.requires_unlocked = true;
    p.track(path)?;
    p.prepare_fastboot()?;
    if live {
        p.add(fastboot(
            d,
            vec!["boot".into(), path.display().to_string()],
            "Temporary boot",
        ));
    } else {
        for slot in slots(d, &options.slot)? {
            let mut args = args_with_options(options);
            if let Some(slot) = slot {
                args.push(format!("--slot={slot}"));
            }
            args.extend(["flash".into(), partition.into(), path.display().to_string()]);
            p.add(fastboot(d, args, "Flash partition"));
        }
        if !options.no_reboot {
            p.add(fastboot(d, vec!["reboot".into()], "Reboot system"));
        }
    }
    Ok(p)
}
pub fn factory(
    d: &Device,
    f: &Firmware,
    options: &FlashOptions,
    root_image: Option<(&Path, &str)>,
) -> Result<Plan> {
    ensure!(
        f.kind == "Factory image",
        "Select a processed factory image package"
    );
    ensure!(
        !f.product.is_empty() && f.product == d.product,
        "Firmware product {} does not match selected device {}",
        f.product,
        d.product
    );
    let script = ["flash-all.bat", "flash-all.sh"]
        .iter()
        .map(|n| f.directory.join(n))
        .find(|p| p.exists())
        .context("Factory flash-all script is missing")?;
    let lines = fs::read_to_string(script)?;
    let commands = parse_factory_script(&lines)?;
    let mut p = Plan::new("Flash factory firmware", d);
    p.destructive = true;
    p.requires_unlocked = true;
    if options.wipe {
        p.warnings.push("All user data will be erased".into());
    }
    if options.disable_verity || options.disable_verification {
        p.warnings
            .push("Android verified boot settings will be changed".into());
    }
    p.prepare_fastboot()?;
    for raw in commands {
        if raw[0] == "reboot-bootloader" {
            p.add(fastboot(d, raw, "Reload bootloader"));
            p.steps.push(Step::WaitFastboot);
            continue;
        }
        let input = raw.last().context("Factory command has no input")?;
        let name = Path::new(input)
            .file_name()
            .context("Factory filename is invalid")?;
        let path = f.directory.join(name);
        p.track(&path)?;
        for slot in slots(d, &options.slot)? {
            let mut args = args_with_options(options);
            if let Some(slot) = slot {
                args.push(format!("--slot={slot}"));
            }
            if options.wipe && raw[0] == "update" {
                args.push("-w".into());
            }
            if raw[0] == "update" {
                args.push("--skip-reboot".into());
            }
            args.extend(raw[..raw.len() - 1].iter().cloned());
            args.push(path.display().to_string());
            p.add(fastboot(d, args, "Flash firmware component"));
        }
    }
    if let Some((path, partition)) = root_image {
        let root = boot_image(
            d,
            path,
            partition,
            false,
            &FlashOptions {
                no_reboot: true,
                ..options.clone()
            },
        )?;
        p.artifacts.extend(root.artifacts);
        p.steps.extend(
            root.steps
                .into_iter()
                .filter(|s| matches!(s,Step::Command(c) if c.tool==Tool::Fastboot)),
        );
    }
    if !options.no_reboot {
        p.add(fastboot(d, vec!["reboot".into()], "Reboot updated device"));
    }
    Ok(p)
}
pub fn parse_factory_script(script: &str) -> Result<Vec<Vec<String>>> {
    let mut commands = vec![];
    for line in script.lines() {
        let line = line.trim();
        if !line.starts_with("fastboot ") {
            continue;
        }
        ensure!(
            !line.contains([';', '&', '|', '>', '<', '`', '$']),
            "Factory script contains unsupported command syntax"
        );
        let mut parts: Vec<String> = line
            .split_whitespace()
            .skip(1)
            .map(|s| s.trim_matches(['\'', '"']).to_owned())
            .collect();
        if parts.first().is_some_and(|v| v == "-w") {
            parts.remove(0);
        }
        match parts.first().map(String::as_str) {
            Some("flash") if parts.len() == 3 => {}
            Some("update") if parts.len() == 2 => {}
            Some("reboot-bootloader") if parts.len() == 1 => {}
            _ => bail!("Unsupported factory command: {line}"),
        };
        commands.push(parts);
    }
    ensure!(
        commands
            .iter()
            .any(|a| a.first().is_some_and(|s| s == "update")),
        "Factory script contains no fastboot update command"
    );
    Ok(commands)
}
pub fn sideload(d: &Device, path: &Path) -> Result<Plan> {
    ensure!(
        d.mode == Mode::Sideload,
        "Device must already be in recovery's Apply update from ADB mode"
    );
    let mut p = Plan::new("Sideload full OTA", d);
    p.destructive = true;
    p.track(path)?;
    let mut c = Invocation::adb(
        &d.serial,
        &["sideload", &path.display().to_string()],
        "Apply OTA",
    );
    c.timeout = 3600;
    p.add(c);
    p.warnings.push("Confirm recovery's result before rebooting; sideload transport completion is not update verification".into());
    Ok(p)
}
pub fn simple(
    d: &Device,
    title: &str,
    args: &[&str],
    destructive: bool,
    unlocked: bool,
) -> Result<Plan> {
    let mut p = Plan::new(title, d);
    p.destructive = destructive;
    p.requires_unlocked = unlocked;
    if d.mode == Mode::Fastboot {
        p.add(Invocation::fastboot(&d.serial, args, title));
    } else {
        p.add(Invocation::adb(&d.serial, args, title));
    }
    Ok(p)
}
fn preflight(t: &dyn Transport, p: &Plan) -> Result<()> {
    let mut call = Invocation::fastboot(
        &p.device.serial,
        &["getvar", "product"],
        "Verify target product",
    );
    call.timeout = 10;
    let vars = parse_variables(&quiet(t, &call)?.text());
    let actual = vars
        .get("product")
        .context("Bootloader did not report a product")?;
    ensure!(
        actual == &p.device.product,
        "Connected device product changed; aborting"
    );
    if p.requires_unlocked {
        call.args = vec![
            "-s".into(),
            p.device.serial.clone(),
            "getvar".into(),
            "unlocked".into(),
        ];
        let v = parse_variables(&quiet(t, &call)?.text());
        ensure!(
            v.get("unlocked").is_some_and(|v| v == "yes" || v == "true"),
            "Bootloader unlock status is not confirmed"
        );
    }
    Ok(())
}
pub fn execute(
    t: &dyn Transport,
    p: &Plan,
    confirmation: &str,
    cancel: &AtomicBool,
    log: &(dyn Fn(String) + Send + Sync),
) -> Result<()> {
    p.validate_confirmation(confirmation)?;
    ensure!(!cancel.load(Ordering::Relaxed), "Operation cancelled");
    for step in &p.steps {
        if let Step::Command(c) = step {
            ensure!(
                !c.needs_secret || c.secret.is_some(),
                "Plan requires fresh human secret input"
            );
            ensure!(
                matches!(c.tool, Tool::Adb | Tool::Fastboot),
                "Device plans may contain only Android platform-tool commands"
            );
            ensure!(
                c.args.len() >= 3 && c.args[0] == "-s" && c.args[1] == p.device.serial,
                "Plan command does not target the reviewed device"
            );
            ensure!(
                !c.args[2..].iter().any(|arg| arg.starts_with("-s")
                    || arg.starts_with("--serial")
                    || arg.starts_with("-t")
                    || arg.starts_with("--transport-id")
                    || arg == "-d"
                    || arg == "-e"),
                "Plan contains overriding device targeting arguments"
            );
        }
    }
    for a in &p.artifacts {
        ensure!(
            image::hashes(&a.path)?.1 == a.sha256,
            "Input changed after review: {}",
            a.path.display()
        );
    }
    ensure!(!p.steps.is_empty(), "Empty operation plan");
    for (package, uid) in &p.expected_uids {
        crate::command::validate_identifier(package)?;
        let found = quiet(
            t,
            &Invocation::adb(
                &p.device.serial,
                &["shell", "pm", "list", "packages", "-U", package],
                "Revalidate package UID",
            ),
        )?;
        ensure!(
            crate::services::parse_packages(&found.stdout)
                .iter()
                .any(|a| a.name == *package && a.uid == *uid),
            "Package UID changed or permission target disappeared; refresh Applications"
        );
    }
    if p.steps
        .iter()
        .any(|step| matches!(step,Step::Command(c) if c.tool==Tool::Fastboot))
    {
        let version = quiet(
            t,
            &Invocation::global(Tool::Fastboot, &["--version"], "Validate fastboot version"),
        )?;
        let parsed = crate::device::platform_version(&version.text())
            .context("Fastboot did not report a recognized platform-tools version")?;
        crate::device::validate_platform_version(parsed)?;
    }
    if p.device.mode == Mode::Adb || p.device.mode == Mode::Recovery {
        let call = Invocation::adb(
            &p.device.serial,
            &["shell", "getprop", "ro.product.device"],
            "Verify Android target",
        );
        let product = quiet(t, &call)?.stdout.trim().to_owned();
        ensure!(
            !p.device.product.is_empty() && product == p.device.product,
            "Android device product changed or is unknown; inspect the target again"
        );
        if let Some(expected) = &p.expected_build {
            let actual = quiet(
                t,
                &Invocation::adb(
                    &p.device.serial,
                    &["shell", "getprop", "ro.build.fingerprint"],
                    "Revalidate backup build",
                ),
            )?;
            ensure!(
                actual.stdout.trim() == expected,
                "Device build changed after backup review"
            );
        }
    } else {
        ensure!(
            p.expected_build.is_none(),
            "Matching restore requires fresh Android build inspection"
        );
    }
    for (index, step) in p.steps.iter().enumerate() {
        ensure!(!cancel.load(Ordering::Relaxed), "Operation cancelled");
        log(format!("Step {} of {}", index + 1, p.steps.len()));
        match step {
            Step::Command(c) => {
                if c.tool == Tool::Fastboot
                    && c.args.iter().any(|v| {
                        matches!(
                            v.as_str(),
                            "flash" | "update" | "boot" | "erase" | "set_active" | "flashing"
                        )
                    })
                {
                    preflight(t, p)?;
                }
                t.execute(c, cancel, log)?.require_success()?;
            }
            Step::WaitFastboot => {
                let start = Instant::now();
                loop {
                    ensure!(
                        !cancel.load(Ordering::Relaxed),
                        "Cancelled waiting for bootloader"
                    );
                    let mut c = Invocation::fastboot(
                        &p.device.serial,
                        &["getvar", "product"],
                        "Waiting for fastboot",
                    );
                    c.timeout = 3;
                    if quiet(t, &c).is_ok() {
                        break;
                    }
                    ensure!(
                        start.elapsed() < Duration::from_secs(90),
                        "Timed out waiting for fastboot"
                    );
                    thread::sleep(Duration::from_millis(500));
                }
            }
            Step::Sleep(s) => thread::sleep(Duration::from_secs(*s)),
        }
    }
    log("All commands completed. Refresh device information to verify final state.".into());
    Ok(())
}
