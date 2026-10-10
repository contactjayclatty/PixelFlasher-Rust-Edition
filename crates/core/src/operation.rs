use crate::{
    command::{Invocation, Output, Tool, Transport},
    device::Device,
    flash::{self, Plan},
    library::Library,
};
use anyhow::{Result, ensure};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    ops::Deref,
    path::Path,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceSnapshot {
    pub device: Device,
    pub captured_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EffectiveOptions {
    pub wipe_userdata: bool,
    pub slots: Vec<String>,
    pub disable_verity: bool,
    pub disable_verification: bool,
    pub force: bool,
    pub final_reboot: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum OperationIntent {
    FlashImage {
        device_id: String,
        artifact_id: String,
        partition: String,
    },
    OpenManager {
        device_id: String,
        provider: crate::root::Solution,
    },
    PatchRoot {
        device_id: String,
        image_id: String,
        manager_id: String,
        provider: crate::root::Solution,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OperationPlan {
    plan: Plan,
    id: String,
    digest: String,
    snapshot: DeviceSnapshot,
}
impl Deref for OperationPlan {
    type Target = Plan;
    fn deref(&self) -> &Plan {
        &self.plan
    }
}
pub struct Approval {
    digest: String,
    phrase: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum JobStatus {
    Prepared,
    Running,
    CommandsCompleted,
    Verified,
    Failed,
    Cancelled,
    Interrupted,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobEvent {
    pub job_id: String,
    pub stage: String,
    pub status: JobStatus,
    pub message: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OperationResult {
    pub id: String,
    pub status: JobStatus,
    pub verification: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OperationRecord {
    pub id: String,
    pub title: String,
    pub status: String,
    pub updated: String,
    pub verification: String,
}
static COUNTER: AtomicU64 = AtomicU64::new(0);
pub fn opaque_id(prefix: &str) -> String {
    let seed = format!(
        "{}:{}:{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    format!(
        "{prefix}-{}",
        hex::encode(Sha256::digest(seed.as_bytes()))[..24].to_owned()
    )
}
impl OperationPlan {
    pub fn freeze(plan: Plan) -> Result<Self> {
        ensure!(
            !plan.device.serial.is_empty(),
            "A reviewed plan needs a selected device"
        );
        let digest = hex::encode(Sha256::digest(serde_json::to_vec(&plan)?));
        let snapshot = DeviceSnapshot {
            device: plan.device.clone(),
            captured_at: chrono::Utc::now().to_rfc3339(),
        };
        Ok(Self {
            plan,
            id: opaque_id("op"),
            digest,
            snapshot,
        })
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn snapshot(&self) -> &DeviceSnapshot {
        &self.snapshot
    }
    pub fn effective_options(&self) -> Option<EffectiveOptions> {
        let calls = self
            .steps
            .iter()
            .filter_map(|step| match step {
                crate::flash::Step::Command(c) if c.tool == Tool::Fastboot => Some(c),
                _ => None,
            })
            .collect::<Vec<_>>();
        if calls.is_empty() {
            return None;
        }
        let has = |flag: &str| calls.iter().any(|c| c.args.iter().any(|v| v == flag));
        let mut slots = std::collections::BTreeSet::new();
        for call in &calls {
            for arg in &call.args {
                if let Some(slot) = arg.strip_prefix("--slot=") {
                    slots.insert(slot.to_owned());
                }
            }
            for args in call.args.windows(2) {
                if args[0] == "set_active" {
                    slots.insert(format!("activate {}", args[1]));
                }
            }
        }
        if slots.is_empty() {
            slots.insert(if self.device.slot.is_empty() {
                "unknown current slot".into()
            } else {
                format!("current ({})", self.device.slot)
            });
        }
        Some(EffectiveOptions {
            wipe_userdata: has("-w")
                || calls.iter().any(|c| {
                    c.args
                        .windows(2)
                        .any(|v| v[0] == "flashing" && matches!(v[1].as_str(), "lock" | "unlock"))
                })
                || calls.iter().any(|c| {
                    c.args
                        .windows(2)
                        .any(|v| v[0] == "erase" && v[1] == "userdata")
                }),
            slots: slots.into_iter().collect(),
            disable_verity: has("--disable-verity"),
            disable_verification: has("--disable-verification"),
            force: has("--force"),
            final_reboot: has("reboot"),
        })
    }
    pub fn approve(&self, phrase: &str) -> Result<Approval> {
        self.validate_confirmation(phrase)?;
        Ok(Approval {
            digest: self.digest.clone(),
            phrase: phrase.into(),
        })
    }
}
pub fn is_critical(call: &Invocation) -> bool {
    call.critical
        || (call.tool == Tool::Fastboot
            && call.args.iter().any(|v| {
                matches!(
                    v.as_str(),
                    "flash" | "update" | "erase" | "flashing" | "set_active"
                )
            }))
        || (call.tool == Tool::Adb
            && (call.args.iter().any(|v| v == "sideload")
                || (call.args.iter().any(|v| v == "su")
                    && call.args.last().is_some_and(|script| {
                        [
                            "touch ",
                            "rm ",
                            "cp ",
                            "tar -xz",
                            "--sqlite",
                            "--denylist",
                            " module ",
                            "action.sh",
                            "of=/dev/block",
                        ]
                        .iter()
                        .any(|v| script.contains(v))
                    }))))
}
struct CheckpointTransport<'a> {
    inner: &'a dyn Transport,
    stop: &'a AtomicBool,
    artifacts: &'a [crate::flash::Artifact],
    expected_slot: std::sync::Mutex<String>,
}
impl Transport for CheckpointTransport<'_> {
    fn execute(
        &self,
        call: &Invocation,
        cancel: &AtomicBool,
        log: &(dyn Fn(String) + Send + Sync),
    ) -> Result<Output> {
        ensure!(
            !self.stop.load(Ordering::Relaxed),
            "Stop requested at checkpoint"
        );
        if is_critical(call) {
            if call.tool == Tool::Adb && call.args.iter().any(|v| v == "su") {
                let uid = self
                    .inner
                    .execute(
                        &Invocation::adb(
                            &call.args[1],
                            &["shell", "su", "-c", "id -u"],
                            "Verify current root permission",
                        ),
                        cancel,
                        log,
                    )?
                    .require_success()?;
                ensure!(
                    uid.stdout.trim() == "0",
                    "Root permission is missing or changed; inspect and review again"
                );
            }
            for artifact in self.artifacts {
                ensure!(
                    crate::image::hashes(&artifact.path)?.1 == artifact.sha256,
                    "Input changed at mutation checkpoint"
                );
            }
            let mut slot = self
                .expected_slot
                .lock()
                .map_err(|_| anyhow::anyhow!("Checkpoint state failed"))?;
            if call.tool == Tool::Fastboot && !slot.is_empty() {
                let actual = self
                    .inner
                    .execute(
                        &Invocation::fastboot(
                            &call.args[1],
                            &["getvar", "current-slot"],
                            "Revalidate reviewed slot",
                        ),
                        cancel,
                        log,
                    )?
                    .require_success()?;
                let vars = crate::device::parse_variables(&actual.text());
                ensure!(
                    vars.get("current-slot") == Some(&*slot),
                    "Device slot changed or could not be verified; inspect and review again"
                );
            }
            log("Critical stage: a stop request takes effect at the next checkpoint".into());
            let output = self.inner.execute(call, &AtomicBool::new(false), log)?;
            if output.code == 0
                && let Some(i) = call.args.iter().position(|v| v == "set_active")
            {
                *slot = call
                    .args
                    .get(i + 1)
                    .filter(|s| s.as_str() == "a" || s.as_str() == "b")
                    .cloned()
                    .unwrap_or_default();
            }
            Ok(output)
        } else {
            self.inner.execute(call, cancel, log)
        }
    }
}
pub fn recover_interrupted(data: &Path) -> Result<usize> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(data.join("operations.lock"))?;
    if file.try_lock_exclusive().is_err() {
        return Ok(0);
    }
    Library::open(data)?.recover_interrupted()
}
pub fn run_approved(
    t: &dyn Transport,
    p: &OperationPlan,
    approval: &Approval,
    data: &Path,
    cancel: &AtomicBool,
    log: &(dyn Fn(String) + Send + Sync),
) -> Result<OperationResult> {
    ensure!(
        approval.digest == p.digest,
        "Approval belongs to a different plan"
    );
    ensure!(
        hex::encode(Sha256::digest(serde_json::to_vec(&p.plan)?)) == p.digest,
        "Frozen plan digest changed"
    );
    let library = Library::open(data)?;
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(data.join("operations.lock"))?;
    lock.try_lock_exclusive()
        .map_err(|_| anyhow::anyhow!("Another device operation is already running"))?;
    library.journal(
        p.id(),
        &p.title,
        &p.digest,
        "Running",
        "Device state has not yet been verified",
    )?;
    let wrapped = CheckpointTransport {
        inner: t,
        stop: cancel,
        artifacts: &p.artifacts,
        expected_slot: std::sync::Mutex::new(p.device.slot.clone()),
    };
    let outcome = flash::execute(&wrapped, &p.plan, &approval.phrase, cancel, log);
    match outcome {
        Ok(()) => {
            library.journal(
                p.id(),
                &p.title,
                &p.digest,
                "CommandsCompleted",
                "Commands completed; inspect final device state",
            )?;
            Ok(OperationResult {
                id: p.id.clone(),
                status: JobStatus::CommandsCompleted,
                verification: "Final device state is not verified".into(),
            })
        }
        Err(error) => {
            let status = if cancel.load(Ordering::Relaxed) {
                "Cancelled"
            } else {
                "Failed"
            };
            library.journal(
                p.id(),
                &p.title,
                &p.digest,
                status,
                "Inspect device state before retry; completed writes are not rolled back",
            )?;
            Err(error)
        }
    }
}
