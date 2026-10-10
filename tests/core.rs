use anyhow::Result;
use pixelflasher_rust::{
    command::{Invocation, Output, Tool, Transport},
    device::{self, Device, Mode},
    firmware,
    flash::{self, FlashOptions, Plan, Slot},
    image,
    library::Library,
    payload::{Extent, Manifest, Operation, Partition, PartitionInfo},
    root, services, xml,
};
use prost::Message;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Write},
    path::Path,
    sync::{Mutex, atomic::AtomicBool},
};

fn boot(path: &Path, kernel: u32) {
    let mut bytes = vec![0u8; 8192];
    bytes[..8].copy_from_slice(b"ANDROID!");
    bytes[8..12].copy_from_slice(&kernel.to_le_bytes());
    bytes[12..16].copy_from_slice(&1024u32.to_le_bytes());
    bytes[20..24].copy_from_slice(&1580u32.to_le_bytes());
    bytes[40..44].copy_from_slice(&4u32.to_le_bytes());
    fs::write(path, bytes).unwrap();
}
fn device() -> Device {
    Device {
        serial: "TEST1234".into(),
        product: "komodo".into(),
        model: "Pixel test fixture".into(),
        mode: Mode::Fastboot,
        slot: "a".into(),
        unlocked: Some(true),
        ..Default::default()
    }
}
struct Fake {
    calls: Mutex<Vec<Invocation>>,
    locked: bool,
    fail: bool,
}
impl Transport for Fake {
    fn execute(
        &self,
        c: &Invocation,
        _: &AtomicBool,
        _: &(dyn Fn(String) + Send + Sync),
    ) -> Result<Output> {
        self.calls.lock().unwrap().push(c.clone());
        let args = &c.args;
        let out = if args.iter().any(|s| s == "--version") {
            "fastboot version 37.0.0-test\n"
        } else if args.iter().any(|s| s == "getvar") {
            if args.last().unwrap() == "product" {
                "(bootloader) product: komodo\n"
            } else if self.locked {
                "(bootloader) unlocked: no\n"
            } else {
                "(bootloader) unlocked: yes\n"
            }
        } else {
            "OKAY\n"
        };
        Ok(Output {
            code: if self.fail && args.iter().any(|s| s == "flash") {
                1
            } else {
                0
            },
            stdout: String::new(),
            stderr: out.into(),
        })
    }
}
fn fake() -> Fake {
    Fake {
        calls: Mutex::new(vec![]),
        locked: false,
        fail: false,
    }
}

#[test]
fn discovery_preserves_modes_and_multiple_devices() {
    let devices = device::parse_adb(
        "List of devices attached\nA device product:komodo model:Pixel_9_Pro_XL transport_id:1\nB unauthorized\nC recovery\nD sideload\nE offline\n",
    );
    assert_eq!(devices.len(), 5);
    assert_eq!(devices[0].model, "Pixel 9 Pro XL");
    assert_eq!(devices[1].mode, Mode::Unauthorized);
    assert_eq!(devices[3].mode, Mode::Sideload);
}
#[test]
fn adb_and_fastboot_properties_parse_without_losing_colon_keys() {
    let p = device::parse_properties("[ro.build.fingerprint]: [google/komodo/komodo:17/test]\n");
    assert_eq!(p["ro.build.fingerprint"], "google/komodo/komodo:17/test");
    let v = device::parse_variables(
        "(bootloader) partition-size:boot_a: 0x04000000\n(bootloader) current-slot: a\nFinished. Total time: 0.2s\n",
    );
    assert_eq!(v["partition-size:boot_a"], "0x04000000");
    assert_eq!(v["current-slot"], "a");
}
#[test]
fn image_metadata_distinguishes_boot_and_init_boot() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("boot.img");
    let b = dir.path().join("init_boot.img");
    boot(&a, 1024);
    boot(&b, 0);
    assert_eq!(image::inspect(&a).unwrap().kind, "boot");
    assert_eq!(image::inspect(&b).unwrap().kind, "init_boot");
    assert_eq!(image::inspect(&a).unwrap().sha256.len(), 64);
    fs::write(&b, b"ANDROID!").unwrap();
    assert!(image::inspect(&b).is_err());
}
#[test]
fn mismatched_image_and_unknown_inactive_slot_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("boot.img");
    boot(&path, 1024);
    assert!(
        flash::boot_image(
            &device(),
            &path,
            "init_boot",
            false,
            &FlashOptions::default()
        )
        .is_err()
    );
    let mut d = device();
    d.slot.clear();
    assert!(
        flash::boot_image(
            &d,
            &path,
            "boot",
            false,
            &FlashOptions {
                slot: Slot::Inactive,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(
        flash::boot_image(
            &device(),
            &path,
            "init_boot",
            true,
            &FlashOptions::default()
        )
        .is_err()
    );
}
#[test]
fn inactive_slot_plan_is_serial_bound_and_does_not_wipe() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("boot.img");
    boot(&path, 1024);
    let p = flash::boot_image(
        &device(),
        &path,
        "boot",
        false,
        &FlashOptions {
            slot: Slot::Inactive,
            no_reboot: true,
            ..Default::default()
        },
    )
    .unwrap();
    let fake = fake();
    flash::execute(
        &fake,
        &p,
        &device().confirmation(),
        &AtomicBool::new(false),
        &|_| {},
    )
    .unwrap();
    let calls = fake.calls.lock().unwrap();
    let c = calls
        .iter()
        .find(|c| c.args.iter().any(|s| s == "flash"))
        .unwrap();
    assert_eq!(&c.args[..2], &["-s", "TEST1234"]);
    assert!(c.args.iter().any(|s| s == "--slot=b"));
    assert!(!c.args.iter().any(|s| s == "-w"));
}
#[test]
fn locked_target_fails_before_any_flash() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("boot.img");
    boot(&path, 1024);
    let p = flash::boot_image(&device(), &path, "boot", false, &FlashOptions::default()).unwrap();
    let fake = Fake {
        locked: true,
        ..fake()
    };
    assert!(
        flash::execute(
            &fake,
            &p,
            &device().confirmation(),
            &AtomicBool::new(false),
            &|_| {}
        )
        .is_err()
    );
    assert!(
        !fake
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.args.iter().any(|s| s == "flash"))
    );
}
#[test]
fn changed_input_and_wrong_confirmation_fail_before_transport() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("boot.img");
    boot(&path, 1024);
    let p = flash::boot_image(&device(), &path, "boot", false, &FlashOptions::default()).unwrap();
    let fake = fake();
    assert!(flash::execute(&fake, &p, "wrong", &AtomicBool::new(false), &|_| {}).is_err());
    fs::write(&path, b"modified after review").unwrap();
    assert!(
        flash::execute(
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
fn transport_failure_stops_before_reboot() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("boot.img");
    boot(&path, 1024);
    let p = flash::boot_image(&device(), &path, "boot", false, &FlashOptions::default()).unwrap();
    let fake = Fake {
        fail: true,
        ..fake()
    };
    assert!(
        flash::execute(
            &fake,
            &p,
            &device().confirmation(),
            &AtomicBool::new(false),
            &|_| {}
        )
        .is_err()
    );
    assert!(
        !fake
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.args.iter().any(|s| s == "reboot"))
    );
}
#[test]
fn factory_scripts_are_parsed_not_executed() {
    let script = "@ECHO OFF\nfastboot flash bootloader bootloader-komodo.img\nfastboot reboot-bootloader\nfastboot flash radio radio-komodo.img\nfastboot -w update image-komodo.zip\n";
    let calls = flash::parse_factory_script(script).unwrap();
    assert_eq!(calls.len(), 4);
    assert_eq!(calls[3][0], "update");
    assert!(flash::parse_factory_script("fastboot update a.zip && erase userdata").is_err());
    assert!(flash::parse_factory_script("fastboot erase userdata").is_err());
}
#[test]
fn nested_factory_archive_extracts_images_and_preserves_provenance() {
    let dir = tempfile::tempdir().unwrap();
    let stock = dir.path().join("stock.img");
    boot(&stock, 0);
    let mut nested = zip::ZipWriter::new(Cursor::new(Vec::new()));
    nested
        .start_file("init_boot.img", zip::write::SimpleFileOptions::default())
        .unwrap();
    nested.write_all(&fs::read(&stock).unwrap()).unwrap();
    let nested = nested.finish().unwrap().into_inner();
    let path = dir.path().join("komodo-test-factory.zip");
    let mut z = zip::ZipWriter::new(fs::File::create(&path).unwrap());
    z.start_file(
        "komodo-test/image-komodo.zip",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    z.write_all(&nested).unwrap();
    z.start_file(
        "komodo-test/flash-all.bat",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    z.write_all(b"@ECHO OFF\nfastboot -w update image-komodo.zip\n")
        .unwrap();
    z.finish().unwrap();
    let data = dir.path().join("data");
    let f = firmware::process(&path, &data, &AtomicBool::new(false), &|_| {}).unwrap();
    assert_eq!(f.product, "komodo");
    assert_eq!(f.images.len(), 1);
    let rows = Library::open(&data).unwrap().list().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].info.kind, "init_boot");
    assert!(rows[0].origin.ends_with("komodo-test-factory.zip"));
    let second = firmware::process(&path, &data, &AtomicBool::new(false), &|_| {}).unwrap();
    assert_eq!(second.directory, f.directory);
}
#[test]
fn archive_path_traversal_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.zip");
    let mut z = zip::ZipWriter::new(fs::File::create(&path).unwrap());
    z.start_file("../boot.img", zip::write::SimpleFileOptions::default())
        .unwrap();
    z.write_all(b"bad").unwrap();
    z.finish().unwrap();
    assert!(
        firmware::process(
            &path,
            &dir.path().join("data"),
            &AtomicBool::new(false),
            &|_| {}
        )
        .is_err()
    );
    assert!(!dir.path().join("boot.img").exists());
}
fn payload_file(path: &Path, corrupt: bool, delta: bool) {
    let mut content = vec![0u8; 4096];
    content[..8].copy_from_slice(b"ANDROID!");
    content[40..44].copy_from_slice(&4u32.to_le_bytes());
    let hash = Sha256::digest(&content).to_vec();
    let manifest = Manifest {
        block_size: Some(4096),
        minor_version: Some(if delta { 1 } else { 0 }),
        partitions: vec![Partition {
            name: "init_boot".into(),
            info: Some(PartitionInfo {
                size: Some(4096),
                hash: Some(hash.clone()),
            }),
            operations: vec![Operation {
                kind: 0,
                offset: Some(0),
                length: Some(4096),
                extents: vec![Extent {
                    start: Some(0),
                    blocks: Some(1),
                }],
                hash: Some(if corrupt { vec![0; 32] } else { hash }),
            }],
        }],
    };
    let bytes = manifest.encode_to_vec();
    let mut file = fs::File::create(path).unwrap();
    file.write_all(b"CrAU").unwrap();
    file.write_all(&2u64.to_be_bytes()).unwrap();
    file.write_all(&(bytes.len() as u64).to_be_bytes()).unwrap();
    file.write_all(&0u32.to_be_bytes()).unwrap();
    file.write_all(&bytes).unwrap();
    file.write_all(&content).unwrap();
}
#[test]
fn payload_extraction_verifies_operation_and_partition_hashes() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("payload.bin");
    payload_file(&p, false, false);
    let out = dir.path().join("out");
    fs::create_dir(&out).unwrap();
    let images =
        pixelflasher_rust::payload::extract(&p, &out, &AtomicBool::new(false), &|_| {}).unwrap();
    assert_eq!(images.len(), 1);
    assert_eq!(fs::metadata(&images[0]).unwrap().len(), 4096);
    assert_eq!(image::inspect(&images[0]).unwrap().kind, "init_boot");
}
#[test]
fn corrupt_and_incremental_payloads_fail() {
    for (corrupt, delta) in [(true, false), (false, true)] {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("payload.bin");
        payload_file(&p, corrupt, delta);
        assert!(
            pixelflasher_rust::payload::extract(&p, dir.path(), &AtomicBool::new(false), &|_| {})
                .is_err()
        );
    }
}
#[test]
fn library_deduplicates_content_and_keeps_patch_lineage() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("init_boot.img");
    boot(&p, 0);
    let info = image::inspect(&p).unwrap();
    let library = Library::open(dir.path()).unwrap();
    library
        .insert(
            &p,
            "firmware.zip",
            "komodo",
            "KernelSU-Next",
            "original_sha1",
            &info,
        )
        .unwrap();
    library.insert(&p, "ignored", "", "", "", &info).unwrap();
    let rows = library.list().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].patched_with, "KernelSU-Next");
    assert_eq!(rows[0].source_sha1, "original_sha1");
    library.remove(rows[0].id).unwrap();
    assert!(library.list().unwrap().is_empty());
}
#[test]
fn unchanged_patch_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.img");
    let b = dir.path().join("b.img");
    boot(&a, 0);
    fs::copy(&a, &b).unwrap();
    assert!(root::validate_patch(&a, &b).is_err());
}
#[test]
fn packages_split_apks_and_module_properties_parse() {
    let p = services::parse_packages("package:/data/app/a/base.apk=com.example.app uid:10234\n");
    assert_eq!(p[0].name, "com.example.app");
    assert_eq!(p[0].uid, "10234");
    let m =
        root::parse_modules("PFR_MODULE\nid=test\nname=Test module\nversion=1\npfr_disabled=1\n");
    assert_eq!(m[0].name, "Test module");
    assert!(m[0].disabled);
}
#[test]
fn wireless_endpoint_and_pairing_code_are_validated_and_redacted() {
    let c = services::wireless("pair", "192.168.1.2:12345", "123456").unwrap();
    assert!(!c.display().contains("123456"));
    assert!(services::wireless("connect", "host; touch /bad", "").is_err());
    assert!(services::wireless("pair", "192.168.1.2:12345", "invalid").is_err());
}
#[test]
fn shell_identifiers_and_escaping_reject_injection() {
    assert!(pixelflasher_rust::command::validate_identifier("com.example.app").is_ok());
    assert!(pixelflasher_rust::command::validate_identifier("app; rm -rf /").is_err());
    assert_eq!(pixelflasher_rust::command::shell_quote("a'b"), "'a'\\''b'");
}
#[test]
fn cancellation_prevents_any_device_command() {
    let p = Plan::new("test", &device());
    let fake = fake();
    assert!(
        flash::execute(
            &fake,
            &p,
            &device().confirmation(),
            &AtomicBool::new(true),
            &|_| {}
        )
        .is_err()
    );
    assert!(fake.calls.lock().unwrap().is_empty());
}
#[test]
fn binary_xml_rejects_bad_chunks_and_plain_xml_roundtrips() {
    assert_eq!(
        xml::decode(b"<manifest package=\"example\"/>").unwrap(),
        "<manifest package=\"example\"/>"
    );
    assert!(xml::decode(&[3, 0, 8, 0, 255, 255, 255, 255]).is_err());
}
#[test]
fn google_catalog_uses_only_known_download_hosts() {
    let text = "<a href=\"https://dl.google.com/dl/android/aosp/komodo-test.zip\">link</a> <code>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa</code>";
    let items = services::parse_google_images(text);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].sha256.as_ref().unwrap().len(), 64);
    assert!(services::parse_google_images("https://evil.example/a.zip").is_empty());
}
#[test]
fn bootloader_serial_is_hidden_in_plan_display() {
    let c = Invocation::fastboot("SECRET_DEVICE", &["flash", "boot", "image.img"], "test");
    assert!(!c.display().contains("SECRET_DEVICE"));
    assert_eq!(c.tool, Tool::Fastboot);
}

#[test]
fn platform_tools_guard_matches_upstream_known_bad_releases() {
    assert!(device::validate_platform_version((33, 0, 2)).is_err());
    assert!(device::validate_platform_version((33, 0, 3)).is_ok());
    for patch in 0..=4 {
        assert!(device::validate_platform_version((34, 0, patch)).is_err());
    }
    assert!(device::validate_platform_version((34, 0, 5)).is_ok());
    assert_eq!(
        device::platform_version("fastboot version 37.0.0-123\n"),
        Some((37, 0, 0))
    );
}

#[test]
fn legacy_database_import_is_read_only_and_checks_content_hash() {
    let dir = tempfile::tempdir().unwrap();
    let image_path = dir.path().join("init_boot.img");
    boot(&image_path, 0);
    let info = image::inspect(&image_path).unwrap();
    let database = dir.path().join("PixelFlasher4.db");
    {
        let db = rusqlite::Connection::open(&database).unwrap();
        db.execute_batch("CREATE TABLE BOOT(file_path TEXT,boot_hash TEXT,hardware TEXT,is_patched INTEGER,magisk_version TEXT,patch_method TEXT);").unwrap();
        db.execute(
            "INSERT INTO BOOT VALUES(?1,?2,'komodo',0,NULL,NULL)",
            rusqlite::params![image_path.display().to_string(), info.sha1],
        )
        .unwrap();
    }
    let before = fs::read(&database).unwrap();
    let lib = Library::open(&dir.path().join("new-data")).unwrap();
    let (imported, skipped) = lib.import_legacy(&database).unwrap();
    assert_eq!(imported, 1);
    assert!(skipped.is_empty());
    assert_eq!(fs::read(&database).unwrap(), before);
    fs::write(&image_path, b"changed").unwrap();
    let (imported, skipped) = lib.import_legacy(&database).unwrap();
    assert_eq!(imported, 0);
    assert_eq!(skipped.len(), 1);
}

#[test]
fn plan_cannot_preflight_one_device_and_write_another() {
    for args in [
        vec!["-s", "OTHER_DEVICE", "flash", "boot", "image.img"],
        vec!["flash", "boot", "image.img"],
        vec![
            "-s",
            "TEST1234",
            "-sOTHER_DEVICE",
            "flash",
            "boot",
            "image.img",
        ],
        vec!["-s", "TEST1234", "-t123", "flash", "boot", "image.img"],
        vec![
            "-s",
            "TEST1234",
            "--transport-id=123",
            "flash",
            "boot",
            "image.img",
        ],
        vec!["-s", "TEST1234", "-d", "flash", "boot", "image.img"],
        vec![
            "-s",
            "TEST1234",
            "-s",
            "OTHER_DEVICE",
            "flash",
            "boot",
            "image.img",
        ],
        vec![
            "-s",
            "TEST1234",
            "--serial=OTHER_DEVICE",
            "flash",
            "boot",
            "image.img",
        ],
    ] {
        let mut plan = Plan::new("fixture", &device());
        plan.add(Invocation {
            tool: Tool::Fastboot,
            args: args.into_iter().map(str::to_owned).collect(),
            directory: None,
            label: "fixture".into(),
            timeout: 1,
            needs_secret: false,
            critical: false,
            secret: None,
        });
        let transport = fake();
        assert!(
            flash::execute(
                &transport,
                &plan,
                &device().confirmation(),
                &AtomicBool::new(false),
                &|_| {}
            )
            .is_err()
        );
        assert!(transport.calls.lock().unwrap().is_empty());
    }
}

#[test]
fn personal_tool_argv_and_echoed_values_are_redacted() {
    let mut call = Invocation::global(Tool::External("personal-tool.exe".into()), &[], "fixture");
    call.args = vec!["--token".into(), "PRIVATE_TOKEN_VALUE".into()];
    assert!(!call.display().contains("PRIVATE_TOKEN_VALUE"));
    assert!(
        !call
            .redact_output("received PRIVATE_TOKEN_VALUE")
            .contains("PRIVATE_TOKEN_VALUE")
    );
}

#[test]
fn avb_properties_are_parsed_and_malformed_lengths_rejected() {
    let key = b"com.android.build.boot.fingerprint";
    let value = b"google/fixture/test:17/build";
    let body = (16 + key.len() + 1 + value.len() + 1).next_multiple_of(8);
    let mut descriptor = vec![0; body + 16];
    descriptor[8..16].copy_from_slice(&(body as u64).to_be_bytes());
    descriptor[16..24].copy_from_slice(&(key.len() as u64).to_be_bytes());
    descriptor[24..32].copy_from_slice(&(value.len() as u64).to_be_bytes());
    descriptor[32..32 + key.len()].copy_from_slice(key);
    descriptor[33 + key.len()..33 + key.len() + value.len()].copy_from_slice(value);
    let (props, count) = image::avb_properties(&descriptor).unwrap();
    assert_eq!(count, 1);
    assert_eq!(
        props[std::str::from_utf8(key).unwrap()],
        std::str::from_utf8(value).unwrap()
    );
    let mut header = vec![0; 256];
    header[..4].copy_from_slice(b"AVB0");
    header[20..28].copy_from_slice(&(descriptor.len() as u64).to_be_bytes());
    header[104..112].copy_from_slice(&(descriptor.len() as u64).to_be_bytes());
    header.extend_from_slice(&descriptor);
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("vbmeta.img");
    fs::write(&file, &header).unwrap();
    assert_eq!(
        image::inspect(&file).unwrap().avb.unwrap().descriptor_count,
        1
    );
    header[104..112].copy_from_slice(&u64::MAX.to_be_bytes());
    fs::write(&file, header).unwrap();
    assert!(image::inspect(&file).is_err());
    descriptor[16..24].copy_from_slice(&u64::MAX.to_be_bytes());
    assert!(image::avb_properties(&descriptor).is_err());
}

#[test]
fn readiness_requires_both_platform_tool_executables() {
    let directory = tempfile::tempdir().unwrap();
    let config = pixelflasher_rust::config::Config {
        platform_tools: directory.path().to_owned(),
        ..Default::default()
    };
    assert!(!config.tools_ready());
    fs::write(
        directory
            .path()
            .join(if cfg!(windows) { "adb.exe" } else { "adb" }),
        b"fixture",
    )
    .unwrap();
    assert!(!config.tools_ready());
    fs::write(
        directory.path().join(if cfg!(windows) {
            "fastboot.exe"
        } else {
            "fastboot"
        }),
        b"fixture",
    )
    .unwrap();
    assert!(config.tools_ready());
}
