use crate::{
    command::Invocation,
    device::{Device, Mode},
    flash::Plan,
    root::{PatchJob, Solution},
};
use anyhow::{Context, Result, ensure};
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

pub fn patch(
    d: &Device,
    stock: &Path,
    kernel: &Path,
    apk: &Path,
    solution: Solution,
    data: &Path,
) -> Result<PatchJob> {
    ensure!(d.mode == Mode::Adb, "GKI patching requires Android mode");
    let info = crate::image::inspect(stock)?;
    let manager = crate::providers::manager_apk(apk)?;
    ensure!(
        manager.provider == solution,
        "Manager APK belongs to a different provider"
    );
    crate::providers::provider(solution).validate_patch(&manager.version, "boot")?;
    ensure!(
        info.kind == "boot",
        "Kernel replacement requires a boot image, not init_boot"
    );
    let arch = d
        .properties
        .get("ro.product.cpu.abi")
        .context("Inspect device architecture first")?;
    crate::command::validate_identifier(arch)?;
    let timestamp = chrono::Utc::now().timestamp_millis();
    let local = data
        .join("patches")
        .join(format!("gki-{timestamp}-{}", &info.sha256[..8]));
    fs::create_dir_all(&local)?;
    let mut zip = zip::ZipArchive::new(File::open(apk)?)?;
    let mut binary = zip
        .by_name(&format!("lib/{arch}/libmagiskboot.so"))
        .or_else(|_| anyhow::bail!("Manager APK has no magiskboot binary for this ABI"))?;
    let magiskboot = local.join("magiskboot");
    std::io::copy(&mut binary, &mut File::create(&magiskboot)?)?;
    let replacement = local.join("replacement-kernel");
    let source = File::open(kernel)?;
    ensure!(
        source.metadata()?.len() <= 512 * 1024 * 1024,
        "Kernel image is too large"
    );
    let mut bytes = vec![];
    if kernel.extension().is_some_and(|v| v == "gz") {
        flate2::read::GzDecoder::new(source)
            .take(512 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
    } else {
        source.take(512 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    }
    ensure!(
        !bytes.is_empty() && bytes.len() <= 512 * 1024 * 1024,
        "Invalid kernel size"
    );
    ensure!(
        !bytes.starts_with(b"ANDROID!"),
        "Select the replacement kernel binary, not another boot image"
    );
    fs::write(&replacement, bytes)?;
    let remote = format!("/data/local/tmp/pfr_gki_{timestamp}_{}", &info.sha256[..8]);
    let script = local.join("pfr_gki.sh");
    fs::write(
        &script,
        format!(
            "#!/system/bin/sh\nset -eu\ncd {}\nchmod 755 magiskboot\n./magiskboot unpack stock.img\ncp replacement-kernel kernel\n./magiskboot repack stock.img\ntest -s new-boot.img\ncp new-boot.img patched.img\n",
            crate::command::shell_quote(&remote)
        ),
    )?;
    let result = local.join("gki-patched.img");
    let mut plan = Plan::new("Replace boot kernel with selected GKI kernel", d);
    plan.destructive = true;
    for p in [stock, kernel, apk, &magiskboot, &replacement, &script] {
        plan.track(p)?;
    }
    plan.warnings.push("The replacement kernel must match the device architecture, Android generation and KMI. This operation does not verify kernel compatibility or flash the image.".into());
    plan.add(Invocation::adb(
        &d.serial,
        &["shell", "mkdir", "-p", &remote],
        "Create kernel patch directory",
    ));
    for (path, name) in [
        (stock, "stock.img"),
        (&magiskboot, "magiskboot"),
        (&replacement, "replacement-kernel"),
        (&script, "pfr_gki.sh"),
    ] {
        plan.add(Invocation::adb(
            &d.serial,
            &[
                "push",
                &path.display().to_string(),
                &format!("{remote}/{name}"),
            ],
            "Transfer kernel patch input",
        ));
    }
    plan.add(Invocation::adb(
        &d.serial,
        &["shell", "sh", &format!("{remote}/pfr_gki.sh")],
        "Repack boot image",
    ));
    plan.add(Invocation::adb(
        &d.serial,
        &[
            "pull",
            &format!("{remote}/patched.img"),
            &result.display().to_string(),
        ],
        "Retrieve kernel-patched image",
    ));
    Ok(PatchJob {
        plan,
        result,
        stock: stock.to_owned(),
        solution,
        provider_version: manager.version,
        method: "GKI kernel replacement; manual compatibility".into(),
    })
}
