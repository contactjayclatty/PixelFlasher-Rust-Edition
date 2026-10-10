use pixelflasher_rust::{
    agent,
    command::{Invocation, Output, SecretInput, Transport},
    config::{Config, SavedDevice},
    device::Device,
    flash::Plan,
    library::Library,
    operation::{self, OperationPlan},
    providers::{self, Confidence},
    root::Solution,
};
use serde_json::json;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
struct Fake {
    calls: Mutex<Vec<String>>,
}
impl Transport for Fake {
    fn execute(
        &self,
        c: &Invocation,
        _: &AtomicBool,
        _: &(dyn Fn(String) + Send + Sync),
    ) -> anyhow::Result<Output> {
        self.calls.lock().unwrap().push(c.label.clone());
        Ok(Output {
            code: 0,
            stdout: "komodo".into(),
            stderr: String::new(),
        })
    }
}
fn device() -> Device {
    Device {
        serial: "FIXTURE1234".into(),
        product: "komodo".into(),
        ..Default::default()
    }
}
fn plan(title: &str) -> OperationPlan {
    let d = device();
    let mut p = Plan::new(title, &d);
    p.add(Invocation::adb(
        &d.serial,
        &["shell", "monkey", "-p", "com.topjohnwu.magisk", "1"],
        "manager",
    ));
    OperationPlan::freeze(p).unwrap()
}
#[test]
fn provider_version_golden_matrix() {
    for (id, pair) in [
        (Solution::Magisk, ["30.7", "30.6"]),
        (Solution::KernelSu, ["3.3.0", "3.2.5"]),
        (Solution::KernelSuNext, ["3.4.1", "3.4.0"]),
        (Solution::SukiSu, ["4.2.0", "4.1.3"]),
        (Solution::WildKsu, ["3.1.2", "3.1.1"]),
        (Solution::APatch, ["11224", "11219"]),
    ] {
        let p = providers::provider(id);
        assert_eq!(p.stable_versions(), pair);
        for v in pair {
            assert_eq!(p.capabilities(v).confidence, Confidence::FixtureTested);
            assert!(p.validate_patch(v, "boot").is_ok());
            assert_eq!(
                p.validate_patch(v, "init_boot").is_ok(),
                id != Solution::APatch
            );
            assert!(p.validate_patch(v, "vendor_boot").is_err());
        }
        assert_eq!(
            p.capabilities("future-canary").confidence,
            Confidence::Unverified
        );
        assert!(p.validate_patch("future-canary", "boot").is_err());
        if id != Solution::Magisk {
            assert!(
                p.capabilities(pair[0])
                    .actions
                    .iter()
                    .find(|a| a.name == "permissions")
                    .unwrap()
                    .manager_handoff
            );
        }
    }
}
#[test]
fn approval_is_bound_to_plan() {
    let dir = tempfile::tempdir().unwrap();
    let one = plan("one");
    let two = plan("two");
    let approval = one.approve(&device().confirmation()).unwrap();
    let fake = Fake {
        calls: Mutex::new(vec![]),
    };
    assert!(
        operation::run_approved(
            &fake,
            &two,
            &approval,
            dir.path(),
            &AtomicBool::new(false),
            &|_| {}
        )
        .is_err()
    );
    assert!(fake.calls.lock().unwrap().is_empty());
    assert!(one.approve("wrong device").is_err());
}
#[test]
fn secret_never_roundtrips_or_logs() {
    let mut c = Invocation::adb("FIXTURE1234", &["shell", "sh", "patch.sh"], "patch");
    c.needs_secret = true;
    c.secret = Some(SecretInput::new("PRIVATE_SUPERKEY".into()));
    let text = serde_json::to_string(&c).unwrap();
    assert!(!text.contains("PRIVATE_SUPERKEY"));
    assert!(!format!("{c:?}").contains("PRIVATE_SUPERKEY"));
    assert_eq!(
        c.redact_output("PRIVATE_SUPERKEY FIXTURE1234"),
        "[secret] [device]"
    );
    let copy: Invocation = serde_json::from_str(&text).unwrap();
    assert!(copy.needs_secret && copy.secret.is_none());
    let mut p = Plan::new("patch", &device());
    p.add(copy);
    let fake = Fake {
        calls: Mutex::new(vec![]),
    };
    assert!(
        pixelflasher_rust::flash::execute(
            &fake,
            &p,
            &device().confirmation(),
            &AtomicBool::new(false),
            &|_| {}
        )
        .is_err()
    );
    assert!(fake.calls.lock().unwrap().is_empty());
}
#[test]
fn interrupted_journal_requires_review_and_never_replays() {
    let dir = tempfile::tempdir().unwrap();
    let l = Library::open(dir.path()).unwrap();
    l.journal("test", "write", "digest", "Running", "pending")
        .unwrap();
    assert_eq!(operation::recover_interrupted(dir.path()).unwrap(), 1);
    assert_eq!(operation::recover_interrupted(dir.path()).unwrap(), 0);
    assert_eq!(l.recent_operations().unwrap()[0].status, "Interrupted");
}
#[test]
fn schema_migration_preserves_legacy_rows_and_makes_backup() {
    let dir = tempfile::tempdir().unwrap();
    let db = rusqlite::Connection::open(dir.path().join("library.db")).unwrap();
    db.execute_batch("CREATE TABLE legacy(value TEXT);INSERT INTO legacy VALUES('preserve');")
        .unwrap();
    drop(db);
    Library::open(dir.path()).unwrap();
    let db = rusqlite::Connection::open(dir.path().join("library.db")).unwrap();
    assert_eq!(
        db.query_row("SELECT value FROM legacy", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "preserve"
    );
    assert_eq!(
        db.query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        2
    );
    assert!(std::fs::read_dir(dir.path()).unwrap().any(|p| {
        p.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("library-pre-v1-")
    }));
}
fn enabled(dir: &std::path::Path) {
    Config {
        mcp_enabled: true,
        devices: vec![SavedDevice {
            serial: device().serial,
            label: "Fixture".into(),
            enabled: true,
        }],
        ..Default::default()
    }
    .save(dir)
    .unwrap();
}
#[test]
fn mcp_is_disabled_by_default() {
    let dir = tempfile::tempdir().unwrap();
    assert!(agent::call(dir.path(), "inspect", json!({})).is_err());
}
#[test]
fn mcp_rejects_execution_shell_secret_and_target_overrides() {
    let dir = tempfile::tempdir().unwrap();
    enabled(dir.path());
    for tool in ["execute", "approve", "shell", "secret", "flash"] {
        assert!(agent::call(dir.path(), tool, json!({})).is_err());
    }
    let id = agent::device_id(&device().serial);
    for extra in ["secret", "command", "serial", "approve"] {
        let mut request =
            json!({"intent":{"kind":"OpenManager","device_id":id,"provider":"Magisk"}});
        request["intent"][extra] = json!("malicious");
        assert!(agent::call(dir.path(), "prepare", request).is_err());
    }
    assert!(agent::call(dir.path(),"prepare",json!({"intent":{"kind":"OpenManager","device_id":"another-device","provider":"Magisk"}})).is_err());
}
#[test]
fn mcp_prepare_only_queues_and_inspection_redacts_resources() {
    let dir = tempfile::tempdir().unwrap();
    enabled(dir.path());
    let inspected = agent::call(dir.path(), "inspect", json!({})).unwrap();
    assert!(!inspected.to_string().contains("FIXTURE1234"));
    let value=agent::call(dir.path(),"prepare",json!({"intent":{"kind":"OpenManager","device_id":agent::device_id(&device().serial),"provider":"Magisk"}})).unwrap();
    assert_eq!(value["execution_allowed"], false);
    let l = Library::open(dir.path()).unwrap();
    assert_eq!(l.review_requests().unwrap().len(), 1);
    assert!(l.recent_operations().unwrap().is_empty());
}
#[test]
fn completed_command_is_not_verified_root() {
    let dir = tempfile::tempdir().unwrap();
    let p = plan("manager");
    let a = p.approve(&device().confirmation()).unwrap();
    let fake = Fake {
        calls: Mutex::new(vec![]),
    };
    let r = operation::run_approved(&fake, &p, &a, dir.path(), &AtomicBool::new(false), &|_| {})
        .unwrap();
    assert_eq!(r.status, operation::JobStatus::CommandsCompleted);
    assert_ne!(r.status, operation::JobStatus::Verified);
}
#[test]
fn cancelled_preparation_does_not_run_commands() {
    let dir = tempfile::tempdir().unwrap();
    let p = plan("manager");
    let a = p.approve(&device().confirmation()).unwrap();
    let fake = Fake {
        calls: Mutex::new(vec![]),
    };
    let cancel = AtomicBool::new(true);
    assert!(operation::run_approved(&fake, &p, &a, dir.path(), &cancel, &|_| {}).is_err());
    assert!(fake.calls.lock().unwrap().is_empty());
    assert!(cancel.load(Ordering::Relaxed));
}
#[test]
fn v2_method_rejects_init_boot_and_unpinned_versions() {
    for (id, version) in [
        (Solution::KernelSu, "3.3.0"),
        (Solution::KernelSuNext, "3.4.1"),
    ] {
        let adapter = providers::provider(id);
        assert!(
            adapter
                .validate_method(version, "boot", providers::PatchMethod::KernelSuV2)
                .is_ok()
        );
        assert!(
            adapter
                .validate_method(version, "init_boot", providers::PatchMethod::KernelSuV2)
                .is_err()
        );
        assert!(
            adapter
                .validate_method(
                    adapter.stable_versions()[1],
                    "boot",
                    providers::PatchMethod::KernelSuV2
                )
                .is_err()
        );
    }
    assert!(
        providers::provider(Solution::Magisk)
            .validate_method("30.7", "boot", providers::PatchMethod::KernelSuV2)
            .is_err()
    );
}
#[test]
fn resume_rejects_wrong_offsets_and_malformed_ranges() {
    assert_eq!(
        pixelflasher_rust::services::validate_resume_range("bytes 100-199/200", 100).unwrap(),
        200
    );
    for text in [
        "bytes 0-199/200",
        "bytes 100-200/200",
        "bytes 200-100/300",
        "bytes 100-199/*",
        "garbage",
    ] {
        assert!(pixelflasher_rust::services::validate_resume_range(text, 100).is_err());
    }
}
#[test]
fn catalog_does_not_borrow_next_rows_checksum() {
    let hash = "a".repeat(64);
    let html = format!(
        "<tr><a href=\"https://dl.google.com/first.zip\">first</a></tr><tr><a href=\"https://dl.google.com/second.zip\">second</a>{hash}</tr>"
    );
    let rows = pixelflasher_rust::services::parse_google_images(&html);
    assert_eq!(rows.len(), 2);
    assert!(rows[0].sha256.is_none());
    assert_eq!(rows[1].sha256.as_deref(), Some(hash.as_str()));
}
struct CriticalFake<'a> {
    stop: &'a AtomicBool,
    writes: Mutex<usize>,
    slot: &'a str,
}
impl Transport for CriticalFake<'_> {
    fn execute(
        &self,
        c: &Invocation,
        cancel: &AtomicBool,
        _: &(dyn Fn(String) + Send + Sync),
    ) -> anyhow::Result<Output> {
        let stdout = if c.args.iter().any(|v| v == "--version") {
            "fastboot version 37.0.0".into()
        } else if c.args.last().is_some_and(|v| v == "current-slot") {
            format!("(bootloader) current-slot: {}", self.slot)
        } else if c.args.last().is_some_and(|v| v == "product") {
            "(bootloader) product: komodo".into()
        } else if c.args.iter().any(|v| v == "flash") {
            assert!(!cancel.load(Ordering::Relaxed));
            *self.writes.lock().unwrap() += 1;
            self.stop.store(true, Ordering::Relaxed);
            assert!(!cancel.load(Ordering::Relaxed));
            String::new()
        } else {
            String::new()
        };
        Ok(Output {
            code: 0,
            stdout,
            stderr: String::new(),
        })
    }
}
fn critical_plan() -> OperationPlan {
    let d = Device {
        mode: pixelflasher_rust::device::Mode::Fastboot,
        slot: "a".into(),
        ..device()
    };
    let mut p = Plan::new("checkpoint", &d);
    p.add(Invocation::fastboot(
        &d.serial,
        &["flash", "boot", "fixture.img"],
        "critical fixture",
    ));
    p.add(Invocation::fastboot(&d.serial, &["reboot"], "later stage"));
    OperationPlan::freeze(p).unwrap()
}
#[test]
fn wrong_slot_stops_before_write() {
    let dir = tempfile::tempdir().unwrap();
    let p = critical_plan();
    let stop = AtomicBool::new(false);
    let fake = CriticalFake {
        stop: &stop,
        writes: Mutex::new(0),
        slot: "b",
    };
    let a = p.approve(&device().confirmation()).unwrap();
    assert!(operation::run_approved(&fake, &p, &a, dir.path(), &stop, &|_| {}).is_err());
    assert_eq!(*fake.writes.lock().unwrap(), 0);
}
#[test]
fn critical_cancellation_finishes_current_write_and_stops_next_stage() {
    let dir = tempfile::tempdir().unwrap();
    let p = critical_plan();
    let stop = AtomicBool::new(false);
    let fake = CriticalFake {
        stop: &stop,
        writes: Mutex::new(0),
        slot: "a",
    };
    let a = p.approve(&device().confirmation()).unwrap();
    assert!(operation::run_approved(&fake, &p, &a, dir.path(), &stop, &|_| {}).is_err());
    assert_eq!(*fake.writes.lock().unwrap(), 1);
    assert_eq!(
        Library::open(dir.path())
            .unwrap()
            .recent_operations()
            .unwrap()[0]
            .status,
        "Cancelled"
    );
}
#[test]
fn backup_manifest_binds_device_build_slot_and_content() {
    let directory = tempfile::tempdir().unwrap();
    let d = Device {
        build: "fixture-build".into(),
        slot: "a".into(),
        ..device()
    };
    let file = directory.path().join("boot_a.img");
    std::fs::write(&file, b"synthetic backup contents").unwrap();
    let hash = pixelflasher_rust::image::hashes(&file).unwrap().1;
    std::fs::write(
        directory.path().join("boot_a.sha256"),
        format!("{hash} device-copy.img"),
    )
    .unwrap();
    let output = pixelflasher_rust::recovery::BackupOutput {
        directory: directory.path().into(),
        device: d.clone(),
        partitions: vec!["boot_a".into()],
    };
    pixelflasher_rust::recovery::finalize(&output).unwrap();
    let text = std::fs::read_to_string(directory.path().join("backup.json")).unwrap();
    assert!(!text.contains("FIXTURE1234"));
    assert!(pixelflasher_rust::recovery::verify(directory.path(), &d).is_ok());
    for wrong in [
        Device {
            serial: "other".into(),
            ..d.clone()
        },
        Device {
            build: "other".into(),
            ..d.clone()
        },
        Device {
            slot: "b".into(),
            ..d.clone()
        },
    ] {
        assert!(pixelflasher_rust::recovery::verify(directory.path(), &wrong).is_err());
    }
    std::fs::write(file, b"corrupted contents").unwrap();
    assert!(pixelflasher_rust::recovery::verify(directory.path(), &d).is_err());
}
#[test]
fn backup_rejects_empty_destinations_bad_names_and_checksum_mismatch() {
    let directory = tempfile::tempdir().unwrap();
    let d = Device {
        rooted: true,
        ..device()
    };
    assert!(
        pixelflasher_rust::recovery::prepare(&d, &["boot_a;reboot".into()], directory.path())
            .is_err()
    );
    assert!(pixelflasher_rust::recovery::prepare(&d, &[], directory.path()).is_err());
    std::fs::write(directory.path().join("boot_a.img"), b"fixture").unwrap();
    std::fs::write(directory.path().join("boot_a.sha256"), "bad hash").unwrap();
    assert!(
        pixelflasher_rust::recovery::finalize(&pixelflasher_rust::recovery::BackupOutput {
            directory: directory.path().into(),
            device: d,
            partitions: vec!["boot_a".into()]
        })
        .is_err()
    );
    assert!(!directory.path().join("backup.json").exists());
}
#[test]
fn stale_uid_fails_before_permission_mutation() {
    let mut p = Plan::new("UID-bound policy", &device());
    p.expected_uids
        .push(("com.fixture.app".into(), "10234".into()));
    p.add(Invocation::adb(
        &device().serial,
        &["shell", "su", "-c", "fixture-only"],
        "never run",
    ));
    let fake = Fake {
        calls: Mutex::new(vec![]),
    };
    assert!(
        pixelflasher_rust::flash::execute(
            &fake,
            &p,
            &device().confirmation(),
            &AtomicBool::new(false),
            &|_| {}
        )
        .is_err()
    );
    assert!(!fake.calls.lock().unwrap().iter().any(|s| s == "never run"));
}
#[test]
fn typed_lineage_survives_restart() {
    let directory = tempfile::tempdir().unwrap();
    let lineage = pixelflasher_rust::library::ArtifactLineage {
        stock_sha256: "a".repeat(64),
        firmware_origin: "fixture firmware".into(),
        provider: Solution::KernelSuNext,
        version: "3.4.1".into(),
        method: "Classic".into(),
    };
    let library = Library::open(directory.path()).unwrap();
    library.set_lineage(&"b".repeat(64), &lineage).unwrap();
    drop(library);
    let db = rusqlite::Connection::open(directory.path().join("library.db")).unwrap();
    let text = db
        .query_row("SELECT metadata FROM lineage", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap();
    let restored: pixelflasher_rust::library::ArtifactLineage =
        serde_json::from_str(&text).unwrap();
    assert_eq!(restored.stock_sha256, lineage.stock_sha256);
    assert_eq!(restored.version, "3.4.1");
}
#[test]
fn mcp_omits_sensitive_labels_and_journal_fields() {
    let directory = tempfile::tempdir().unwrap();
    Config {
        mcp_enabled: true,
        devices: vec![SavedDevice {
            serial: device().serial,
            label: "FIXTURE1234 C:/private/token SECRET_CREDENTIAL".into(),
            enabled: true,
        }],
        ..Default::default()
    }
    .save(directory.path())
    .unwrap();
    let library = Library::open(directory.path()).unwrap();
    library
        .journal(
            &operation::opaque_id("op"),
            "SECRET_CREDENTIAL",
            "digest",
            "Failed",
            "C:/private/token FIXTURE1234",
        )
        .unwrap();
    for tool in ["inspect", "status"] {
        let output = agent::call(directory.path(), tool, json!({}))
            .unwrap()
            .to_string();
        for value in ["FIXTURE1234", "SECRET_CREDENTIAL", "C:/private"] {
            assert!(!output.contains(value));
        }
    }
}
#[test]
fn root_reads_remain_interruptible_but_policy_writes_are_critical() {
    assert!(!operation::is_critical(&Invocation::adb(
        "fixture",
        &["shell", "su", "-c", "id -u"],
        "read"
    )));
    assert!(!operation::is_critical(&Invocation::adb(
        "fixture",
        &["shell", "su", "-c", "cat /data/adb/modules/a/module.prop"],
        "read"
    )));
    assert!(operation::is_critical(&Invocation::adb(
        "fixture",
        &["shell", "su", "-c", "touch /data/adb/modules/a/disable"],
        "write"
    )));
}
#[test]
fn all_provider_module_installers_mark_privileged_stage_critical() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("module.zip");
    std::fs::write(&file, b"fixture - never executed").unwrap();
    let d = Device {
        rooted: true,
        ..device()
    };
    for provider in Solution::ALL {
        let plan = pixelflasher_rust::root::install_module(&d, &file, provider).unwrap();
        let calls = plan
            .steps
            .iter()
            .filter_map(|step| {
                if let pixelflasher_rust::flash::Step::Command(call) = step {
                    Some(call)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert!(!operation::is_critical(calls[0]));
        assert!(
            calls[1].critical && operation::is_critical(calls[1]),
            "{}",
            provider.label()
        );
    }
}
#[test]
fn review_uses_effective_frozen_options() {
    let mut p = Plan::new("review", &device());
    p.add(Invocation::fastboot(
        &device().serial,
        &[
            "--slot=b",
            "--disable-verification",
            "--disable-verity",
            "--force",
            "-w",
            "update",
            "fixture.zip",
        ],
        "fixture",
    ));
    let frozen = OperationPlan::freeze(p).unwrap();
    let options = frozen.effective_options().unwrap();
    assert!(
        options.wipe_userdata
            && options.disable_verity
            && options.disable_verification
            && options.force
    );
    assert!(!options.final_reboot);
    assert_eq!(options.slots, vec!["b"]);
    for action in ["lock", "unlock"] {
        let mut p = Plan::new("bootloader", &device());
        p.add(Invocation::fastboot(
            &device().serial,
            &["flashing", action],
            "fixture",
        ));
        assert!(
            OperationPlan::freeze(p)
                .unwrap()
                .effective_options()
                .unwrap()
                .wipe_userdata
        );
    }
    let mut p = Plan::new(
        "slot",
        &Device {
            slot: "a".into(),
            ..device()
        },
    );
    p.add(Invocation::fastboot(
        &device().serial,
        &["set_active", "b"],
        "fixture",
    ));
    assert_eq!(
        OperationPlan::freeze(p)
            .unwrap()
            .effective_options()
            .unwrap()
            .slots,
        vec!["activate b"]
    );
}
#[test]
fn manager_apk_golden_pairs_install_only_the_selected_provider() {
    use std::io::Write;
    let directory = tempfile::tempdir().unwrap();
    for solution in Solution::ALL {
        let adapter = providers::provider(solution);
        for version in adapter.stable_versions() {
            let file = directory
                .path()
                .join(format!("{}-{version}.apk", solution.label()));
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&file).unwrap());
            zip.start_file(
                "AndroidManifest.xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            write!(
                zip,
                "<manifest package=\"{}\" versionName=\"{version}\"/>",
                solution.package()
            )
            .unwrap();
            zip.finish().unwrap();
            let manager = providers::manager_apk(&file).unwrap();
            assert_eq!(manager.provider, solution);
            assert_eq!(manager.version, *version);
            let plan = adapter.manager_install_plan(&device(), &file).unwrap();
            assert!(plan.title.contains(version));
            let wrong = Solution::ALL
                .into_iter()
                .find(|id| *id != solution)
                .unwrap();
            assert!(
                providers::provider(wrong)
                    .manager_install_plan(&device(), &file)
                    .is_err()
            );
        }
    }
}
