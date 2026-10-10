use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub platform_tools: PathBuf,
    pub scrcpy: PathBuf,
    pub expert: bool,
    pub dark: bool,
    pub root_solution: String,
    pub timeout_seconds: u64,
    pub last_firmware: PathBuf,
    pub devices: Vec<SavedDevice>,
    pub tools: Vec<PersonalTool>,
    pub compact: bool,
    pub mcp_enabled: bool,
    pub google_terms_accepted: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedDevice {
    pub serial: String,
    pub label: String,
    pub enabled: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersonalTool {
    pub name: String,
    pub executable: PathBuf,
    pub args: Vec<String>,
    pub directory: PathBuf,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            platform_tools: discover_tools(),
            scrcpy: find_executable("scrcpy").unwrap_or_default(),
            expert: false,
            dark: true,
            root_solution: "KernelSU-Next".into(),
            timeout_seconds: 300,
            last_firmware: PathBuf::new(),
            devices: vec![],
            tools: vec![],
            compact: false,
            mcp_enabled: false,
            google_terms_accepted: false,
        }
    }
}
pub fn data_dir() -> PathBuf {
    if let Some(value) = std::env::var_os("PIXELFLASHER_DATA_DIR") {
        return PathBuf::from(value);
    }
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_DATA_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".local/share")
        });
    base.join("PixelFlasherRust")
}
pub fn normalize_json(text: &str) -> Result<String> {
    let value: FiniteValue = json5::from_str(text).context("Invalid JSON / JSON5 configuration")?;
    serde_json::to_string_pretty(&value.0).context("Configuration cannot be represented as JSON")
}
struct FiniteValue(serde_json::Value);
impl<'de> serde::Deserialize<'de> for FiniteValue {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = FiniteValue;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a finite JSON value")
            }
            fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Self::Value, E> {
                Ok(FiniteValue(serde_json::Value::Null))
            }
            fn visit_bool<E: serde::de::Error>(
                self,
                v: bool,
            ) -> std::result::Result<Self::Value, E> {
                Ok(FiniteValue(v.into()))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> std::result::Result<Self::Value, E> {
                Ok(FiniteValue(v.into()))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> std::result::Result<Self::Value, E> {
                Ok(FiniteValue(v.into()))
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> std::result::Result<Self::Value, E> {
                serde_json::Number::from_f64(v)
                    .map(|v| FiniteValue(v.into()))
                    .ok_or_else(|| E::custom("Non-finite numbers cannot be saved as JSON"))
            }
            fn visit_str<E: serde::de::Error>(
                self,
                v: &str,
            ) -> std::result::Result<Self::Value, E> {
                Ok(FiniteValue(v.into()))
            }
            fn visit_string<E: serde::de::Error>(
                self,
                v: String,
            ) -> std::result::Result<Self::Value, E> {
                Ok(FiniteValue(v.into()))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut values = vec![];
                while let Some(v) = seq.next_element::<FiniteValue>()? {
                    values.push(v.0);
                }
                Ok(FiniteValue(values.into()))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut values = serde_json::Map::new();
                while let Some((key, value)) = map.next_entry::<String, FiniteValue>()? {
                    if values.insert(key, value.0).is_some() {
                        return Err(serde::de::Error::custom("Duplicate configuration key"));
                    }
                }
                Ok(FiniteValue(values.into()))
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}
pub fn find_executable(name: &str) -> Option<PathBuf> {
    let file = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.into()
    };
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|p| p.join(&file))
        .find(|p| p.is_file())
}
fn discover_tools() -> PathBuf {
    for name in ["fastboot", "adb"] {
        if let Some(path) = find_executable(name) {
            let parent = path.parent().unwrap();
            if parent
                .join(if cfg!(windows) { "adb.exe" } else { "adb" })
                .is_file()
                && parent
                    .join(if cfg!(windows) {
                        "fastboot.exe"
                    } else {
                        "fastboot"
                    })
                    .is_file()
            {
                return parent.to_owned();
            }
        }
    }
    PathBuf::new()
}
impl Config {
    pub fn tools_ready(&self) -> bool {
        self.platform_tools
            .join(if cfg!(windows) { "adb.exe" } else { "adb" })
            .is_file()
            && self
                .platform_tools
                .join(if cfg!(windows) {
                    "fastboot.exe"
                } else {
                    "fastboot"
                })
                .is_file()
    }
    pub fn device_label(&self, serial: &str, fallback: &str) -> String {
        self.devices
            .iter()
            .find(|d| d.serial == serial && !d.label.trim().is_empty())
            .map(|d| d.label.clone())
            .unwrap_or_else(|| fallback.into())
    }
    pub fn device_enabled(&self, serial: &str) -> bool {
        self.devices
            .iter()
            .find(|d| d.serial == serial)
            .is_none_or(|d| d.enabled)
    }
    pub fn load(directory: &Path) -> Result<Self> {
        let file = directory.join("settings.json");
        if !file.exists() {
            return Ok(Self::default());
        }
        serde_json::from_slice(&fs::read(file)?)
            .context("Settings JSON is invalid; existing file has been preserved")
    }
    pub fn save(&self, directory: &Path) -> Result<()> {
        fs::create_dir_all(directory)?;
        let temp = directory.join("settings.json.partial");
        fs::write(&temp, serde_json::to_vec_pretty(self)?)?;
        fs::rename(temp, directory.join("settings.json"))?;
        Ok(())
    }
}
