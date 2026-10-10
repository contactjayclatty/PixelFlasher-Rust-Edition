use crate::{
    command::{Invocation, Transport, quiet},
    device::Device,
    flash::Plan,
    root::Solution,
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Confidence {
    Unknown,
    Conflicting,
    Unverified,
    FixtureTested,
    HardwareVerified,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum PatchMethod {
    #[default]
    Classic,
    KernelSuV2,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Capability {
    pub name: String,
    pub available: bool,
    pub reason: String,
    pub manager_handoff: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RootCapabilities {
    pub provider: Solution,
    pub version: String,
    pub stable_versions: Vec<String>,
    pub confidence: Confidence,
    pub targets: Vec<String>,
    pub actions: Vec<Capability>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagerEvidence {
    pub provider: Solution,
    pub package: String,
    pub version: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RootContext {
    pub managers: Vec<ManagerEvidence>,
    pub daemon: String,
    pub kernel: String,
    pub confidence: Confidence,
    pub active_provider: Option<Solution>,
}
pub trait RootProvider: Send + Sync {
    fn id(&self) -> Solution;
    fn stable_versions(&self) -> &'static [&'static str];
    fn validate_method(&self, version: &str, kind: &str, method: PatchMethod) -> Result<()> {
        self.validate_patch(version, kind)?;
        if method == PatchMethod::KernelSuV2 {
            ensure!(
                matches!(
                    (self.id(), version.trim_start_matches('v')),
                    (Solution::KernelSu, "3.3.0") | (Solution::KernelSuNext, "3.4.1")
                ),
                "V2 method is outside the pinned method matrix"
            );
            ensure!(
                kind == "boot",
                "KernelSU V2 always patches boot, never init_boot or vendor_boot"
            );
        }
        Ok(())
    }
    fn patch_script(
        &self,
        version: &str,
        kind: &str,
        assets: &std::path::Path,
        kmi: &str,
        method: PatchMethod,
    ) -> Result<String> {
        self.validate_method(version, kind, method)?;
        match self.id() {
            Solution::Magisk => {
                ensure!(
                    assets.join("boot_patch.sh").is_file() && assets.join("magiskboot").is_file(),
                    "Magisk APK is missing patch assets"
                );
                Ok("export KEEPVERITY=true\nexport KEEPFORCEENCRYPT=true\nexport RECOVERYMODE=false\nsh ./boot_patch.sh stock.img\ntest -s new-boot.img\ncp new-boot.img patched.img\n".into())
            }
            Solution::APatch => {
                ensure!(
                    assets.join("boot_patch.sh").is_file(),
                    "APatch APK is missing boot_patch.sh"
                );
                Ok("IFS= read -r APATCH_SUPERKEY\nsh ./boot_patch.sh \"$APATCH_SUPERKEY\" stock.img\nunset APATCH_SUPERKEY\ntest -s new-boot.img\ncp new-boot.img patched.img\n".into())
            }
            _ => {
                let daemon = if assets.join("ksud").is_file() {
                    "ksud"
                } else if self.id() == Solution::SukiSu && assets.join("zakozako").is_file() {
                    "zakozako"
                } else {
                    anyhow::bail!("APK has no compatible provider daemon")
                };
                if method == PatchMethod::KernelSuV2 {
                    ensure!(
                        kmi.is_empty(),
                        "V2 uses embedded module selection; KMI override is unavailable"
                    );
                    return Ok(format!(
                        "./{daemon} boot-patch-v2 --boot stock.img --output patched.img\n"
                    ));
                }
                Ok(format!(
                    "./{daemon} boot-patch -b stock.img{}\nset -- kernelsu_patched_*.img kernelsu_boot_*.img sukisu_patched_*.img\nfound=0\nfor f in \"$@\"; do\n  if [ -s \"$f\" ]; then cp \"$f\" patched.img; found=$((found+1)); fi\ndone\n[ \"$found\" -eq 1 ]\n",
                    if kmi.is_empty() {
                        String::new()
                    } else {
                        format!(" --kmi {}", crate::command::shell_quote(kmi))
                    }
                ))
            }
        }
    }
    fn capabilities(&self, version: &str) -> RootCapabilities {
        let stable = self.stable_versions();
        let known = stable.iter().any(|s| *s == version.trim_start_matches('v'));
        let actions = [
            "patch",
            "modules",
            "manager_install",
            "recovery",
            "permissions",
        ]
        .into_iter()
        .map(|name| {
            let handoff = name == "permissions" && self.id() != Solution::Magisk;
            Capability {
                name: name.into(),
                available: known,
                manager_handoff: handoff,
                reason: if !known {
                    "Release is outside the pinned stable matrix".into()
                } else if handoff {
                    "Open the official manager for this provider's permission controls".into()
                } else {
                    "Adapter available; hardware validation remains pending".into()
                },
            }
        })
        .collect();
        RootCapabilities {
            provider: self.id(),
            version: version.into(),
            stable_versions: stable.iter().map(|s| (*s).into()).collect(),
            confidence: if known {
                Confidence::FixtureTested
            } else {
                Confidence::Unverified
            },
            targets: if self.id() == Solution::APatch {
                vec!["boot".into()]
            } else {
                vec!["boot".into(), "init_boot".into()]
            },
            actions,
        }
    }
    fn validate_patch(&self, version: &str, image_kind: &str) -> Result<()> {
        let caps = self.capabilities(version);
        ensure!(
            caps.confidence == Confidence::FixtureTested,
            "Provider release is unverified; use a pinned stable manager APK"
        );
        ensure!(
            caps.targets.iter().any(|v| v == image_kind),
            "{} does not support {} with this patch method",
            self.id().label(),
            image_kind
        );
        Ok(())
    }
    fn manager_plan(&self, device: &Device) -> Result<Plan> {
        let mut plan = Plan::new(&format!("Open {} manager", self.id().label()), device);
        plan.add(Invocation::adb(
            &device.serial,
            &[
                "shell",
                "monkey",
                "-p",
                self.id().package(),
                "-c",
                "android.intent.category.LAUNCHER",
                "1",
            ],
            "Open official root manager",
        ));
        Ok(plan)
    }
    fn manager_install_plan(&self, device: &Device, apk: &std::path::Path) -> Result<Plan> {
        let manager = manager_apk(apk)?;
        ensure!(
            manager.provider == self.id(),
            "Manager APK belongs to a different provider"
        );
        ensure!(
            self.stable_versions()
                .contains(&manager.version.trim_start_matches('v')),
            "Manager version is outside the pinned stable matrix"
        );
        let mut plan = crate::services::install_apk(device, apk, false, false)?;
        plan.title = format!("Install {} {} manager", self.id().label(), manager.version);
        Ok(plan)
    }
}
macro_rules! adapter {
    ($name:ident,$id:ident,$one:literal,$two:literal) => {
        pub struct $name;
        impl RootProvider for $name {
            fn id(&self) -> Solution {
                Solution::$id
            }
            fn stable_versions(&self) -> &'static [&'static str] {
                &[$one, $two]
            }
        }
    };
}
adapter!(MagiskProvider, Magisk, "30.7", "30.6");
adapter!(KernelSuProvider, KernelSu, "3.3.0", "3.2.5");
adapter!(KernelSuNextProvider, KernelSuNext, "3.4.1", "3.4.0");
adapter!(SukiSuProvider, SukiSu, "4.2.0", "4.1.3");
adapter!(WildKsuProvider, WildKsu, "3.1.2", "3.1.1");
adapter!(APatchProvider, APatch, "11224", "11219");
pub fn provider(id: Solution) -> Box<dyn RootProvider> {
    match id {
        Solution::Magisk => Box::new(MagiskProvider),
        Solution::KernelSu => Box::new(KernelSuProvider),
        Solution::KernelSuNext => Box::new(KernelSuNextProvider),
        Solution::SukiSu => Box::new(SukiSuProvider),
        Solution::WildKsu => Box::new(WildKsuProvider),
        Solution::APatch => Box::new(APatchProvider),
    }
}
pub fn manager_apk(path: &std::path::Path) -> Result<ManagerEvidence> {
    let xml = crate::advanced::apk_manifest(path)?;
    let mut reader = quick_xml::Reader::from_str(&xml);
    loop {
        match reader.read_event()? {
            quick_xml::events::Event::Start(e) | quick_xml::events::Event::Empty(e)
                if e.name().as_ref() == b"manifest" =>
            {
                let (mut package, mut version) = (String::new(), String::new());
                for attr in e.attributes() {
                    let a = attr?;
                    let key = std::str::from_utf8(a.key.as_ref())?;
                    let value = a.unescape_value()?.into_owned();
                    if key == "package" {
                        package = value;
                    } else if key.ends_with(":versionName") || key == "versionName" {
                        version = value;
                    }
                }
                let solution = Solution::ALL
                    .into_iter()
                    .find(|s| s.package() == package)
                    .ok_or_else(|| anyhow::anyhow!("APK is not a recognized root manager"))?;
                ensure!(
                    !version.is_empty(),
                    "Manager APK does not declare a version"
                );
                return Ok(ManagerEvidence {
                    provider: solution,
                    package,
                    version,
                });
            }
            quick_xml::events::Event::Eof => anyhow::bail!("APK manifest is missing"),
            _ => {}
        }
    }
}
pub fn inspect(t: &dyn Transport, d: &Device) -> Result<RootContext> {
    let text = quiet(
        t,
        &Invocation::adb(
            &d.serial,
            &["shell", "pm", "list", "packages"],
            "Identify installed root managers",
        ),
    )?
    .stdout;
    let mut managers = vec![];
    for id in Solution::ALL {
        if text
            .lines()
            .any(|l| l.trim() == format!("package:{}", id.package()))
        {
            let details = quiet(
                t,
                &Invocation::adb(
                    &d.serial,
                    &["shell", "dumpsys", "package", id.package()],
                    "Read manager version",
                ),
            )?
            .stdout;
            let version = details
                .lines()
                .find_map(|l| l.trim().strip_prefix("versionName="))
                .unwrap_or("")
                .to_owned();
            managers.push(ManagerEvidence {
                provider: id,
                package: id.package().into(),
                version,
            });
        }
    }
    // An installed manager cannot identify a running kernel fork.
    let active = None;
    Ok(RootContext {
        confidence: if managers.len() > 1 {
            Confidence::Conflicting
        } else {
            Confidence::Unknown
        },
        managers,
        daemon: d.root_solution.clone(),
        kernel: d.detail("ro.kernel.version"),
        active_provider: active,
    })
}
