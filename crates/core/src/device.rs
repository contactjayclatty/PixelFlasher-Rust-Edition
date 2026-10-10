use crate::command::{Invocation, Tool, Transport, quiet};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Adb,
    Recovery,
    Sideload,
    Fastboot,
    Unauthorized,
    Offline,
}
impl Mode {
    pub fn label(&self) -> &str {
        match self {
            Self::Adb => "Android",
            Self::Recovery => "Recovery",
            Self::Sideload => "Sideload",
            Self::Fastboot => "Fastboot",
            Self::Unauthorized => "Unauthorized",
            Self::Offline => "Offline",
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Device {
    pub serial: String,
    pub mode: Mode,
    pub model: String,
    pub product: String,
    pub build: String,
    pub slot: String,
    pub unlocked: Option<bool>,
    pub rooted: bool,
    pub root_solution: String,
    pub properties: BTreeMap<String, String>,
    pub variables: BTreeMap<String, String>,
}
impl Device {
    pub fn name(&self) -> String {
        if self.model.is_empty() {
            self.product.clone()
        } else {
            self.model.clone()
        }
    }
    pub fn detail(&self, key: &str) -> String {
        self.properties
            .get(key)
            .or_else(|| self.variables.get(key))
            .cloned()
            .unwrap_or_else(|| "Unknown".into())
    }
    pub fn confirmation(&self) -> String {
        format!(
            "{} {}",
            if self.product.is_empty() {
                "device"
            } else {
                &self.product
            },
            self.serial
                .chars()
                .rev()
                .take(4)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>()
        )
    }
}
pub fn parse_adb(text: &str) -> Vec<Device> {
    text.lines()
        .filter_map(|line| {
            let parts: Vec<_> = line.split_whitespace().collect();
            if parts.len() < 2 || parts[0] == "List" || line.starts_with('*') {
                return None;
            }
            let mode = match parts[1] {
                "device" => Mode::Adb,
                "recovery" => Mode::Recovery,
                "sideload" => Mode::Sideload,
                "unauthorized" => Mode::Unauthorized,
                "offline" => Mode::Offline,
                _ => return None,
            };
            Some(Device {
                serial: parts[0].into(),
                mode,
                model: parts
                    .iter()
                    .find_map(|p| p.strip_prefix("model:"))
                    .unwrap_or("")
                    .replace('_', " "),
                product: parts
                    .iter()
                    .find_map(|p| p.strip_prefix("product:"))
                    .unwrap_or("")
                    .into(),
                ..Default::default()
            })
        })
        .collect()
}
pub fn parse_properties(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|l| {
            l.strip_prefix('[')?
                .split_once("]: [")
                .map(|(k, v)| (k.into(), v.trim_end_matches(']').into()))
        })
        .collect()
}
pub fn parse_variables(text: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim().trim_start_matches("(bootloader)").trim();
        if let Some((key, value)) = line.split_once(": ")
            && !key.starts_with("Finished")
            && !key.starts_with("total time")
        {
            map.insert(key.trim().into(), value.trim().into());
        }
    }
    map
}
pub fn scan(t: &dyn Transport) -> Result<Vec<Device>> {
    let mut devices = parse_adb(
        &quiet(
            t,
            &Invocation::global(Tool::Adb, &["devices", "-l"], "Discover Android devices"),
        )?
        .stdout,
    );
    if let Ok(out) = quiet(
        t,
        &Invocation::global(Tool::Fastboot, &["devices"], "Discover fastboot devices"),
    ) {
        for line in out.stdout.lines() {
            let s: Vec<_> = line.split_whitespace().collect();
            if s.len() >= 2 && s[1] == "fastboot" && !devices.iter().any(|d| d.serial == s[0]) {
                devices.push(Device {
                    serial: s[0].into(),
                    mode: Mode::Fastboot,
                    ..Default::default()
                });
            }
        }
    }
    Ok(devices)
}
pub fn inspect(t: &dyn Transport, mut d: Device) -> Result<Device> {
    match d.mode {
        Mode::Adb | Mode::Recovery => {
            d.properties = parse_properties(
                &quiet(
                    t,
                    &Invocation::adb(&d.serial, &["shell", "getprop"], "Read device properties"),
                )?
                .stdout,
            );
            let get = |k: &str| d.properties.get(k).cloned().unwrap_or_default();
            d.product = get("ro.product.device");
            d.model = get("ro.product.model");
            d.build = get("ro.build.fingerprint");
            d.slot = get("ro.boot.slot_suffix").trim_start_matches('_').into();
            d.unlocked = match get("ro.boot.flash.locked").as_str() {
                "0" => Some(true),
                "1" => Some(false),
                _ => None,
            };
            if let Ok(out) = quiet(
                t,
                &Invocation::adb(&d.serial, &["shell", "su", "-c", "id"], "Check root access"),
            ) {
                d.rooted = out.stdout.contains("uid=0");
            }
            if d.rooted {
                for (tool, path) in [
                    ("KernelSU-family", "/data/adb/ksud"),
                    ("APatch", "/data/adb/apd"),
                    ("Magisk", "/data/adb/magisk/magisk"),
                ] {
                    if let Ok(o) = quiet(
                        t,
                        &Invocation::adb(
                            &d.serial,
                            &["shell", "su", "-c", &format!("test -x {path} && {path} -V")],
                            "Read root runtime",
                        ),
                    ) && !o.stdout.trim().is_empty()
                    {
                        d.root_solution = format!("{tool} {}", o.stdout.trim());
                        break;
                    }
                }
            }
        }
        Mode::Fastboot => {
            d.variables = parse_variables(
                &quiet(
                    t,
                    &Invocation::fastboot(
                        &d.serial,
                        &["getvar", "all"],
                        "Read bootloader variables",
                    ),
                )?
                .text(),
            );
            d.product = d.detail("product");
            d.model = d.product.clone();
            d.slot = d.variables.get("current-slot").cloned().unwrap_or_default();
            d.build = d.detail("version-bootloader");
            d.unlocked = d.variables.get("unlocked").and_then(|s| match s.as_str() {
                "yes" | "true" => Some(true),
                "no" | "false" => Some(false),
                _ => None,
            });
        }
        _ => {}
    }
    Ok(d)
}
pub fn platform_version(text: &str) -> Option<(u32, u32, u32)> {
    text.lines().find_map(|line| {
        let value = line
            .trim()
            .strip_prefix("Version ")
            .or_else(|| line.trim().strip_prefix("fastboot version "))?;
        let v: Vec<_> = value
            .split(['.', '-'])
            .take(3)
            .filter_map(|s| s.parse().ok())
            .collect();
        if v.len() == 3 {
            Some((v[0], v[1], v[2]))
        } else {
            None
        }
    })
}
pub fn validate_platform_version(version: (u32, u32, u32)) -> Result<()> {
    anyhow::ensure!(
        version >= (33, 0, 3),
        "Android platform-tools 33.0.3 or newer are required"
    );
    anyhow::ensure!(
        !(version.0 == 34 && version.1 == 0 && version.2 <= 4),
        "Upstream marks platform-tools 34.0.0 through 34.0.4 as problematic; select another release"
    );
    Ok(())
}
