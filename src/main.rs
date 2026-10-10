#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod ui;
use anyhow::Result;
use pixelflasher_rust::{config, firmware, image, library::Library};

fn main() {
    if let Err(e) = run() {
        eprintln!("{e:#}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let directory = config::data_dir();
    if args.get(1).is_some_and(|s| s == "--catalog") {
        let kind = args.get(2).map(String::as_str).unwrap_or("");
        anyhow::ensure!(
            matches!(kind, "factory" | "ota"),
            "Usage: --catalog factory|ota --acknowledge-google-terms"
        );
        let accepted = args.iter().any(|a| a == "--acknowledge-google-terms");
        println!(
            "{}",
            serde_json::to_string_pretty(&pixelflasher_rust::services::firmware_catalog(
                kind == "ota",
                accepted
            )?)?
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|s| s == "--import-legacy") {
        let path = std::path::Path::new(
            args.get(2)
                .ok_or_else(|| anyhow::anyhow!("Usage: --import-legacy PixelFlasher4.db"))?,
        );
        let (count, skipped) = Library::open(&directory)?.import_legacy(path)?;
        println!(
            "{}",
            serde_json::json!({"imported":count,"skipped":skipped})
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|s| s == "--inspect") {
        let path = std::path::Path::new(
            args.get(2)
                .ok_or_else(|| anyhow::anyhow!("Usage: --inspect image.img"))?,
        );
        println!("{}", serde_json::to_string_pretty(&image::inspect(path)?)?);
        return Ok(());
    }
    if args.get(1).is_some_and(|s| s == "--process") {
        let path = std::path::Path::new(
            args.get(2)
                .ok_or_else(|| anyhow::anyhow!("Usage: --process firmware.zip"))?,
        );
        let result = firmware::process(
            path,
            &directory,
            &std::sync::atomic::AtomicBool::new(false),
            &|s| eprintln!("{s}"),
        )?;
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }
    if args.get(1).is_some_and(|s| s == "--library") {
        println!(
            "{}",
            serde_json::to_string_pretty(&Library::open(&directory)?.list()?)?
        );
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1340.0, 900.0])
            .with_min_inner_size([1024.0, 720.0]),
        ..Default::default()
    };
    eframe::run_native(
        "PixelFlasher Rust Edition",
        options,
        Box::new(move |cc| Ok(Box::new(ui::PixelFlasher::new(cc, directory)))),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(())
}
