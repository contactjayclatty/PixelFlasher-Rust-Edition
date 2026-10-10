use pixelflasher_rust::{
    advanced,
    command::Tool,
    config::{Config, PersonalTool, SavedDevice},
    device::{Device, Mode},
    root::{self, Solution},
};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};
fn device() -> Device {
    Device {
        serial: "FIXTURE1234".into(),
        product: "komodo".into(),
        mode: Mode::Adb,
        rooted: true,
        properties: [("ro.product.cpu.abi".into(), "arm64-v8a".into())].into(),
        ..Default::default()
    }
}
#[test]
fn public_certificate_inspection_omits_private_key() {
    let cert = include_str!("fixtures/upstream-public-certificate.pem");
    let input = format!(
        "<Keybox><PrivateKey>DO_NOT_EXPORT_THIS</PrivateKey><Certificate>{cert}</Certificate></Keybox>"
    );
    let report = advanced::keybox_certificates(&input, None).unwrap();
    assert_eq!(report.len(), 1);
    let json = serde_json::to_string(&report).unwrap();
    assert!(!json.contains("DO_NOT_EXPORT_THIS"));
    assert!(!report[0].serial.is_empty());
    assert_eq!(report[0].revocation, "Not checked");
    assert_eq!(report[0].chain_signature_valid, None);
}
#[test]
fn malformed_certificate_is_rejected() {
    assert!(
        advanced::keybox_certificates("<Certificate>not-a-certificate</Certificate>", None)
            .is_err()
    );
    assert!(advanced::keybox_certificates("<PrivateKey>secret</PrivateKey>", None).is_err());
}
#[test]
fn status_data_is_matched_by_normalized_serial() {
    let cert = include_str!("fixtures/upstream-public-certificate.pem");
    let input = format!("<Certificate>{cert}</Certificate>");
    let initial = advanced::keybox_certificates(&input, None).unwrap();
    let statuses = serde_json::json!({"entries":{format!("00{}",initial[0].serial):{"status":"REVOKED","reason":"test fixture"}}});
    let report = advanced::keybox_certificates(&input, Some(&statuses)).unwrap();
    assert!(report[0].revocation.starts_with("REVOKED"));
}
#[test]
fn root_restore_rejects_outside_data_adb() {
    let dir = tempfile::tempdir().unwrap();
    for (name, valid) in [
        ("adb/modules/test/module.prop", true),
        ("system/override", false),
    ] {
        let path = dir
            .path()
            .join(if valid { "good.tar.gz" } else { "bad.tar.gz" });
        let gz = flate2::write::GzEncoder::new(
            fs::File::create(&path).unwrap(),
            flate2::Compression::default(),
        );
        let mut tar = tar::Builder::new(gz);
        let mut header = tar::Header::new_gnu();
        header.set_size(4);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, name, &b"test"[..]).unwrap();
        tar.into_inner().unwrap().finish().unwrap();
        assert_eq!(advanced::validate_root_archive(&path).is_ok(), valid);
    }
}
#[test]
fn su_policy_and_backup_identifiers_cannot_inject_sql_or_shell() {
    let d = device();
    assert!(
        advanced::magisk_permission(&d, "123; DELETE FROM policies", 2, 0, true, true).is_err()
    );
    assert!(advanced::delete_magisk_backup(&d, "../../other").is_err());
    assert!(advanced::denylist(&d, "app; command", true).is_err());
    let plan = advanced::magisk_permission(&d, "10123", 2, 10, true, true).unwrap();
    assert!(plan.destructive);
    assert_eq!(plan.steps.len(), 1);
}
#[test]
fn support_zip_redacts_serial_and_excludes_tool_arguments() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("support.zip");
    let config = Config {
        devices: vec![SavedDevice {
            serial: "SECRET_SERIAL".into(),
            label: "Test phone".into(),
            enabled: true,
        }],
        tools: vec![PersonalTool {
            name: "test".into(),
            executable: PathBuf::from("tool"),
            args: vec!["SECRET_TOKEN".into()],
            directory: dir.path().to_owned(),
        }],
        ..Default::default()
    };
    advanced::support_zip(&path, &["Connected SECRET_SERIAL".into()], &config).unwrap();
    let mut zip = zip::ZipArchive::new(fs::File::open(&path).unwrap()).unwrap();
    for name in ["settings-redacted.json", "session.log"] {
        let mut text = String::new();
        zip.by_name(name)
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert!(!text.contains("SECRET_SERIAL"));
        assert!(!text.contains("SECRET_TOKEN"));
    }
}
#[test]
fn root_patch_plan_is_native_serial_bound_and_does_not_flash() {
    let dir = tempfile::tempdir().unwrap();
    let stock = dir.path().join("init_boot.img");
    let mut image = vec![0; 4096];
    image[..8].copy_from_slice(b"ANDROID!");
    image[40..44].copy_from_slice(&4u32.to_le_bytes());
    fs::write(&stock, image).unwrap();
    let apk = dir.path().join("manager.apk");
    let mut writer = zip::ZipWriter::new(fs::File::create(&apk).unwrap());
    writer
        .start_file(
            "AndroidManifest.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    writer
        .write_all(br#"<manifest package="com.rifsxd.ksunext" android:versionName="3.4.1"/>"#)
        .unwrap();
    for name in ["lib/arm64-v8a/libksud.so", "lib/arm64-v8a/libmagiskboot.so"] {
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"fixture asset - never executed").unwrap();
    }
    writer.finish().unwrap();
    let patch = root::patch(
        &device(),
        &stock,
        &apk,
        Solution::KernelSuNext,
        "",
        "",
        &dir.path().join("data"),
    )
    .unwrap();
    assert!(!patch.result.exists());
    assert!(patch.plan.artifacts.len() >= 4);
    for step in &patch.plan.steps {
        if let pixelflasher_rust::flash::Step::Command(call) = step {
            assert_eq!(call.tool, Tool::Adb);
            assert_eq!(&call.args[..2], &["-s", "FIXTURE1234"]);
            assert!(!call.args.iter().any(|s| s == "flash"));
        }
    }
    let scripts = patch
        .plan
        .artifacts
        .iter()
        .filter(|a| a.path.file_name().is_some_and(|n| n == "pfr_patch.sh"))
        .collect::<Vec<_>>();
    assert_eq!(scripts.len(), 1);
    let script = fs::read_to_string(&scripts[0].path).unwrap();
    assert!(script.contains("boot-patch -b stock.img"));
    assert!(!script.contains("python"));
}

#[test]
fn supplied_issuer_validates_certificate_chain() {
    let leaf = include_str!("fixtures/upstream-public-certificate.pem");
    let issuer = include_str!("fixtures/upstream-public-root.pem");
    let xml = format!("<Certificate>{leaf}</Certificate><Certificate>{issuer}</Certificate>");
    let report = advanced::keybox_certificates(&xml, None).unwrap();
    assert_eq!(report.len(), 2);
    assert_eq!(report[0].chain_signature_valid, Some(true));
    assert_eq!(report[1].chain_signature_valid, Some(true));
}

#[test]
fn root_configuration_editor_blocks_path_traversal() {
    let dir = tempfile::tempdir().unwrap();
    let d = device();
    assert!(
        pixelflasher_rust::services::rooted_file_write(
            &d,
            "/data/adb/../../system/file",
            "text",
            dir.path()
        )
        .is_err()
    );
    assert!(
        pixelflasher_rust::services::rooted_file_write(
            &d,
            "/data/adb/module/../file",
            "text",
            dir.path()
        )
        .is_err()
    );
    assert!(
        pixelflasher_rust::services::rooted_file_write(&d, "/data/adb/pif.json", "{}", dir.path())
            .is_ok()
    );
}

#[test]
fn json5_editor_preserves_values_and_rejects_silent_loss() {
    let source = "{ // comment\n model: 'Pixel', enabled: true, nested: [1, 0x10,], }";
    let canonical = pixelflasher_rust::config::normalize_json(source).unwrap();
    let value: serde_json::Value = serde_json::from_str(&canonical).unwrap();
    assert_eq!(value["model"], "Pixel");
    assert_eq!(value["nested"][1], 16);
    for text in [
        "{value: NaN}",
        "{value: Infinity}",
        "{value: 1, value: 2}",
        "{broken:",
    ] {
        assert!(
            pixelflasher_rust::config::normalize_json(text).is_err(),
            "{text}"
        );
    }
    assert!(pixelflasher_rust::config::normalize_json("{label:'NaN'}").is_ok());
}

#[test]
fn json5_device_save_stages_valid_json_instead_of_comments() {
    let directory = tempfile::tempdir().unwrap();
    let plan = pixelflasher_rust::services::rooted_file_write(
        &device(),
        "/data/adb/pif.json",
        "{ model:'Pixel', }",
        directory.path(),
    )
    .unwrap();
    let staged = fs::read_to_string(&plan.artifacts[0].path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&staged).unwrap();
    assert_eq!(value["model"], "Pixel");
    assert!(
        pixelflasher_rust::services::rooted_file_write(
            &device(),
            "/data/adb/pif.json",
            "{broken:",
            directory.path()
        )
        .is_err()
    );
}
