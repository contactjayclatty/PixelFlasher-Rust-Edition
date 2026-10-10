use anyhow::{Result, ensure};
use eframe::egui::{self, Color32, RichText, Vec2};
use pixelflasher_rust::{
    advanced,
    command::{self, Invocation, ProcessTransport, Tool, Transport},
    config::{Config, PersonalTool},
    device::{self, Device, Mode},
    firmware::{self, Firmware},
    flash::{self, FlashOptions, Plan, Slot, Step},
    image,
    library::{BootRecord, Library},
    operation::{self, OperationPlan},
    providers,
    root::{self, Module, Solution},
    services::{self, Download, Package},
    xml,
};
use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{Duration, Instant},
};
mod advanced_pages;
mod pages;
mod readiness;
mod studio;

const ACCENT: Color32 = Color32::from_rgb(128, 218, 188);
const MUTED: Color32 = Color32::from_rgb(143, 156, 169);
const CARD: Color32 = Color32::from_rgb(27, 34, 43);
const BORDER: Color32 = Color32::from_rgb(49, 59, 70);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Overview,
    Firmware,
    Flash,
    Apps,
    Root,
    Backups,
    Wireless,
    Utilities,
    Settings,
}
impl Page {
    const ALL: [Self; 9] = [
        Self::Overview,
        Self::Firmware,
        Self::Flash,
        Self::Apps,
        Self::Root,
        Self::Backups,
        Self::Wireless,
        Self::Utilities,
        Self::Settings,
    ];
    fn name(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Firmware => "Firmware & Images",
            Self::Flash => "Update Studio",
            Self::Apps => "Applications",
            Self::Root => "Root Studio",
            Self::Backups => "Backups & Recovery",
            Self::Wireless => "Wireless",
            Self::Utilities => "Diagnostics",
            Self::Settings => "Settings",
        }
    }
    fn subtitle(self) -> &'static str {
        match self {
            Self::Overview => "Your device, firmware and next operation in one place.",
            Self::Firmware => "Inspect packages. Keep stock and patched images organized.",
            Self::Flash => "Build a plan, review every command, then confirm the device.",
            Self::Apps => "Manage installed packages without leaving the workspace.",
            Self::Root => "Patch images and manage modules for your selected root solution.",
            Self::Backups => "Preserve partitions and root data before making changes.",
            Self::Wireless => "Pair and connect using Android's wireless debugging.",
            Self::Utilities => "Device diagnostics, configuration editing and image inspection.",
            Self::Settings => "Configure platform tools and workspace preferences.",
        }
    }
}
enum Payload {
    Devices(Vec<Device>),
    Device(Device),
    Firmware(Firmware),
    Apps(Vec<Package>),
    Modules(Vec<Module>),
    Partitions(Vec<String>),
    Backups(Vec<String>),
    Downloads(Vec<Download>),
    Text(String),
    FileText {
        serial: String,
        path: String,
        text: String,
    },
    InspectionText(String),
    LibraryChanged(String),
    Review(Box<Pending>),
    RootContext(providers::RootContext),
    Finished,
    Patched,
}
enum Event {
    Log(String),
    Result(std::result::Result<Payload, String>),
}
#[derive(Clone)]
struct PatchResult {
    stock: PathBuf,
    result: PathBuf,
    solution: Solution,
    provider_version: String,
    method: String,
}
struct Pending {
    plan: OperationPlan,
    patch: Option<PatchResult>,
    backup: Option<pixelflasher_rust::recovery::BackupOutput>,
}
pub struct PixelFlasher {
    data: PathBuf,
    config: Config,
    page: Page,
    devices: Vec<Device>,
    selected: Option<usize>,
    images: Vec<BootRecord>,
    image: Option<i64>,
    firmware: Option<Firmware>,
    packages: Vec<Package>,
    selected_packages: BTreeSet<String>,
    modules: Vec<Module>,
    partitions: Vec<String>,
    selected_partition: String,
    backup_partitions: BTreeSet<String>,
    magisk_backups: Vec<String>,
    log: Vec<String>,
    error: Option<String>,
    status: String,
    busy: bool,
    cancel: Arc<AtomicBool>,
    tx: Sender<Event>,
    rx: Receiver<Event>,
    pending: Option<Pending>,
    confirmation: String,
    acknowledge: bool,
    firmware_path: String,
    tools_path: String,
    scrcpy_path: String,
    search: String,
    options: FlashOptions,
    partition: String,
    live: bool,
    downloads: Vec<Download>,
    download_filter: String,
    ota_catalog: bool,
    solution: Solution,
    root_apk: String,
    kernel_image: String,
    kmi: String,
    superkey: String,
    endpoint: String,
    pair_code: String,
    file_path: String,
    file_content: String,
    utility_command: String,
    show_console: bool,
    utility_json: String,
    tool_name: String,
    tool_path: String,
    tool_args: String,
    include_disabled: bool,
    image_filter: String,
    only_patched: bool,
    job_started: Option<Instant>,
    job_progress: Option<f32>,
    install_store_owner: bool,
    install_bypass: bool,
    root_context: Option<providers::RootContext>,
    root_tab: usize,
    update_tab: usize,
    root_version: String,
    patch_method: providers::PatchMethod,
}
impl PixelFlasher {
    pub fn new(cc: &eframe::CreationContext<'_>, data: PathBuf) -> Self {
        Self::from_context(&cc.egui_ctx, data)
    }
    fn from_context(ctx: &egui::Context, data: PathBuf) -> Self {
        let (tx, rx) = mpsc::channel();
        let settings = Config::load(&data);
        let error = settings.as_ref().err().map(|e| format!("{e:#}"));
        let config = settings.unwrap_or_default();
        let solution = Solution::from_label(&config.root_solution);
        theme(ctx, config.dark);
        let _ = Library::open(&data).and_then(|_| operation::recover_interrupted(&data));
        let images = Library::open(&data)
            .and_then(|l| l.list())
            .unwrap_or_default();
        Self {
            tools_path: config.platform_tools.display().to_string(),
            scrcpy_path: config.scrcpy.display().to_string(),
            firmware_path: config.last_firmware.display().to_string(),
            config,
            data,
            page: match std::env::var("PIXELFLASHER_CAPTURE_PAGE")
                .unwrap_or_default()
                .as_str()
            {
                "firmware" => Page::Firmware,
                "flash" => Page::Flash,
                "apps" => Page::Apps,
                "root" => Page::Root,
                "backups" => Page::Backups,
                "wireless" => Page::Wireless,
                "utilities" => Page::Utilities,
                "settings" => Page::Settings,
                _ => Page::Overview,
            },
            devices: vec![],
            selected: None,
            images,
            image: None,
            firmware: None,
            packages: vec![],
            selected_packages: BTreeSet::new(),
            modules: vec![],
            partitions: vec![],
            selected_partition: String::new(),
            backup_partitions: BTreeSet::new(),
            magisk_backups: vec![],
            log: vec!["Workspace initialized. Device commands run only when requested.".into()],
            error,
            status: "Ready".into(),
            busy: false,
            cancel: command::cancellation(),
            tx,
            rx,
            pending: None,
            confirmation: String::new(),
            acknowledge: false,
            search: String::new(),
            options: FlashOptions::default(),
            partition: "init_boot".into(),
            live: false,
            downloads: vec![],
            download_filter: String::new(),
            ota_catalog: false,
            solution,
            root_apk: String::new(),
            kernel_image: String::new(),
            kmi: String::new(),
            superkey: String::new(),
            endpoint: String::new(),
            pair_code: String::new(),
            file_path: "/data/adb/pif.json".into(),
            file_content: String::new(),
            utility_command: "getprop ro.build.fingerprint".into(),
            show_console: false,
            utility_json: String::new(),
            tool_name: String::new(),
            tool_path: String::new(),
            tool_args: String::new(),
            include_disabled: false,
            image_filter: String::new(),
            only_patched: false,
            job_started: None,
            job_progress: None,
            install_store_owner: false,
            install_bypass: false,
            patch_method: providers::PatchMethod::Classic,
            root_context: None,
            root_tab: 0,
            update_tab: 0,
            root_version: providers::provider(solution).stable_versions()[0].into(),
        }
    }
    fn transport(&self) -> ProcessTransport {
        ProcessTransport {
            directory: self.config.platform_tools.clone(),
        }
    }
    fn device(&self) -> Result<Device> {
        self.selected
            .and_then(|i| self.devices.get(i))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Select and inspect a connected device first"))
    }
    fn selected_image(&self) -> Result<BootRecord> {
        self.images
            .iter()
            .find(|b| Some(b.id) == self.image)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Select an image from the library"))
    }
    fn refresh_library(&mut self) {
        match Library::open(&self.data).and_then(|l| l.list()) {
            Ok(v) => self.images = v,
            Err(e) => self.error = Some(format!("{e:#}")),
        }
    }
    fn run<F>(&mut self, label: &str, job: F)
    where
        F: FnOnce(ProcessTransport, Arc<AtomicBool>, Sender<Event>) -> Result<Payload>
            + Send
            + 'static,
    {
        if self.busy {
            self.error = Some("An operation is already running".into());
            return;
        }
        self.busy = true;
        self.job_started = Some(Instant::now());
        self.job_progress = None;
        self.show_console = true;
        self.status = label.into();
        self.error = None;
        self.cancel = command::cancellation();
        let cancel = self.cancel.clone();
        let tx = self.tx.clone();
        let transport = self.transport();
        thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                job(transport, cancel, tx.clone())
            }))
            .map_err(|_| "Worker failed unexpectedly".to_owned())
            .and_then(|r| r.map_err(|e| format!("{e:#}")));
            let _ = tx.send(Event::Result(result));
        });
    }
    fn command(&mut self, call: Invocation) {
        self.run("Running command", move |t, c, tx| {
            let logger = move |line| {
                let _ = tx.send(Event::Log(line));
            };
            let result = t.execute(&call, &c, &logger)?.require_success()?;
            Ok(Payload::Text(result.text()))
        });
    }
    fn scan(&mut self) {
        self.run("Discovering devices", |t, _, _| {
            Ok(Payload::Devices(device::scan(&t)?))
        });
    }
    fn inspect(&mut self) {
        match self.device() {
            Ok(d) => self.run("Inspecting device", move |t, _, _| {
                Ok(Payload::Device(device::inspect(&t, d)?))
            }),
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    fn prepare(&mut self, result: Result<Plan>) {
        match result {
            Ok(plan) => {
                let plan = match OperationPlan::freeze(plan) {
                    Ok(plan) => plan,
                    Err(e) => {
                        self.error = Some(e.to_string());
                        return;
                    }
                };
                let _ = Library::open(&self.data).and_then(|l| {
                    l.journal(
                        plan.id(),
                        &plan.title,
                        plan.digest(),
                        "Prepared",
                        "Awaiting local human review",
                    )
                });
                self.pending = Some(Pending {
                    plan,
                    patch: None,
                    backup: None,
                });
                self.confirmation.clear();
                self.acknowledge = false;
            }
            Err(e) => self.error = Some(format!("{e:#}")),
        }
    }
    fn prepare_async<F>(&mut self, label: &str, job: F)
    where
        F: FnOnce() -> Result<Pending> + Send + 'static,
    {
        self.run(label, move |_, cancel, _| {
            let pending = job()?;
            ensure!(
                !cancel.load(Ordering::Relaxed),
                "Plan preparation cancelled"
            );
            Ok(Payload::Review(Box::new(pending)))
        });
    }
    fn run_pending(&mut self) {
        if let Some(pending) = self.pending.take() {
            let phrase = self.confirmation.clone();
            let data = self.data.clone();
            self.run(&pending.plan.title.clone(), move |t, c, tx| {
                let logger = move |line| {
                    let _ = tx.send(Event::Log(line));
                };
                let approval = pending.plan.approve(&phrase)?;
                let outcome =
                    operation::run_approved(&t, &pending.plan, &approval, &data, &c, &logger);
                let library = Library::open(&data)?;
                library.record_operation(
                    &pending.plan.title,
                    if outcome.is_ok() {
                        "commands completed"
                    } else {
                        "failed"
                    },
                    &format!("{} steps", pending.plan.steps.len()),
                )?;
                outcome?;
                if let Some(backup) = pending.backup {
                    pixelflasher_rust::recovery::finalize(&backup)?;
                }
                if let Some(patch) = pending.patch {
                    let info = root::validate_patch(&patch.stock, &patch.result)?;
                    let stock = image::inspect(&patch.stock)?;
                    library.insert(
                        &patch.result,
                        &patch.stock.display().to_string(),
                        &pending.plan.device.product,
                        patch.solution.label(),
                        &stock.sha1,
                        &info,
                    )?;
                    library.set_lineage(
                        &info.sha256,
                        &pixelflasher_rust::library::ArtifactLineage {
                            stock_sha256: stock.sha256.clone(),
                            firmware_origin: library
                                .list()?
                                .iter()
                                .find(|r| r.info.sha256 == stock.sha256)
                                .map(|r| r.origin.clone())
                                .unwrap_or_else(|| "User-selected stock image".into()),
                            provider: patch.solution,
                            version: patch.provider_version,
                            method: patch.method,
                        },
                    )?;
                    Ok(Payload::Patched)
                } else {
                    Ok(Payload::Finished)
                }
            });
        }
    }
    fn poll(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Log(line) => {
                    if let Some((current, total)) = line
                        .strip_prefix("Step ")
                        .and_then(|s| s.split_once(" of "))
                        && let (Ok(current), Ok(total)) =
                            (current.parse::<f32>(), total.parse::<f32>())
                        && total > 0.0
                    {
                        self.job_progress = Some(((current - 1.0) / total).clamp(0.0, 1.0));
                    }
                    self.log.push(line);
                    if self.log.len() > 2000 {
                        self.log.drain(..500);
                    }
                }
                Event::Result(result) => {
                    self.busy = false;
                    self.job_progress = None;
                    match result {
                        Err(e) => {
                            self.log.push(format!("ERROR: {e}"));
                            self.error = Some(e);
                            self.status = "Needs attention".into();
                        }
                        Ok(payload) => {
                            self.status = "Ready".into();
                            match payload {
                                Payload::Devices(v) => {
                                    let previous = self
                                        .selected
                                        .and_then(|i| self.devices.get(i))
                                        .map(|d| d.serial.clone());
                                    for d in &v {
                                        if !self
                                            .config
                                            .devices
                                            .iter()
                                            .any(|saved| saved.serial == d.serial)
                                        {
                                            self.config.devices.push(
                                                pixelflasher_rust::config::SavedDevice {
                                                    serial: d.serial.clone(),
                                                    label: d.name(),
                                                    enabled: true,
                                                },
                                            );
                                        }
                                    }
                                    let _ = self.config.save(&self.data);
                                    self.devices = v
                                        .into_iter()
                                        .filter(|d| {
                                            self.include_disabled
                                                || self.config.device_enabled(&d.serial)
                                        })
                                        .collect();
                                    self.selected = previous.as_ref().and_then(|s| {
                                        self.devices.iter().position(|d| &d.serial == s)
                                    });
                                    self.clear_device_caches();
                                    self.log.push(format!(
                                        "Found {} connected devices",
                                        self.devices.len()
                                    ));
                                }
                                Payload::Device(d) => {
                                    if let Some(i) =
                                        self.devices.iter().position(|v| v.serial == d.serial)
                                    {
                                        self.devices[i] = d;
                                    }
                                }
                                Payload::Firmware(f) => {
                                    self.firmware = Some(f);
                                    self.refresh_library();
                                }
                                Payload::Apps(p) => self.packages = p,
                                Payload::Modules(m) => self.modules = m,
                                Payload::Partitions(p) => self.partitions = p,
                                Payload::Backups(p) => self.magisk_backups = p,
                                Payload::Downloads(d) => self.downloads = d,
                                Payload::FileText { serial, path, text } => {
                                    if self.device().is_ok_and(|d| d.serial == serial)
                                        && self.file_path == path
                                    {
                                        self.file_content = text;
                                    } else {
                                        self.error = Some(
                                            "Configuration target or source changed; read it again"
                                                .into(),
                                        );
                                    }
                                }
                                Payload::InspectionText(text) => self.utility_json = text,
                                Payload::RootContext(context) => self.root_context = Some(context),
                                Payload::Review(pending) => {
                                    let _ = Library::open(&self.data).and_then(|l| {
                                        l.journal(
                                            pending.plan.id(),
                                            &pending.plan.title,
                                            pending.plan.digest(),
                                            "Prepared",
                                            "Awaiting local human review",
                                        )
                                    });
                                    self.pending = Some(*pending);
                                    self.confirmation.clear();
                                    self.acknowledge = false;
                                }
                                Payload::LibraryChanged(message) => {
                                    self.refresh_library();
                                    self.log.push(message);
                                }
                                Payload::Text(text) => {
                                    for line in text.lines() {
                                        let mut safe = line.to_owned();
                                        for d in &self.devices {
                                            safe = safe.replace(&d.serial, "[device]");
                                        }
                                        self.log.push(safe);
                                    }
                                    self.show_console = true;
                                }
                                Payload::Finished => {
                                    self.log.push("Command sequence completed. Inspect final device state before further operations.".into());
                                }
                                Payload::Patched => {
                                    self.refresh_library();
                                    self.log.push(
                                        "Patched image validated and added to the library.".into(),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    fn clear_device_caches(&mut self) {
        if let Some(pending) = self.pending.take() {
            let _ = Library::open(&self.data).and_then(|l| {
                l.journal(
                    pending.plan.id(),
                    &pending.plan.title,
                    pending.plan.digest(),
                    "Cancelled",
                    "Target refreshed or changed; prepare a fresh review",
                )
            });
        }
        self.confirmation.clear();
        self.acknowledge = false;
        self.options = FlashOptions::default();

        self.root_context = None;
        self.file_content.clear();
        self.utility_json.clear();
        self.superkey.clear();
        self.packages.clear();
        self.selected_packages.clear();
        self.modules.clear();
        self.partitions.clear();
        self.backup_partitions.clear();
        self.selected_partition.clear();
        self.magisk_backups.clear();
    }
    fn process_dropped_file(&mut self, ctx: &egui::Context) {
        let paths = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect::<Vec<_>>()
        });
        if paths.is_empty() {
            return;
        }
        if self.busy {
            self.error = Some("Wait for the current operation before dropping another file".into());
            return;
        }
        let path = paths[0].clone();
        if path
            .extension()
            .is_some_and(|v| v.eq_ignore_ascii_case("apk"))
        {
            self.root_apk = path.display().to_string();
            self.page = Page::Root;
            self.status = "Manager APK selected; no device action performed".into();
            return;
        }
        let data = self.data.clone();
        self.firmware_path = path.display().to_string();
        self.config.last_firmware = path.clone();
        let _ = self.config.save(&self.data);
        self.page = Page::Firmware;
        self.run("Processing dropped firmware", move |_, cancel, tx| {
            Ok(Payload::Firmware(firmware::process(
                &path,
                &data,
                &cancel,
                &|line| {
                    let _ = tx.send(Event::Log(line));
                },
            )?))
        });
    }
    fn sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("navigation")
            .exact_width(210.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(if self.config.dark {
                        Color32::from_rgb(15, 21, 28)
                    } else {
                        Color32::from_rgb(229, 237, 231)
                    })
                    .inner_margin(20),
            )
            .show(ctx, |ui| {
                ui.add_space(10.0);
                ui.label(
                    RichText::new("PIXELFLASHER")
                        .size(17.0)
                        .strong()
                        .color(accent(ui)),
                );
                ui.label(RichText::new("RUST EDITION").size(10.0).color(muted(ui)));
                ui.add_space(35.0);
                for (i, page) in Page::ALL.iter().enumerate() {
                    if i == 3 {
                        ui.add_space(18.0);
                        ui.label(RichText::new("DEVICE TOOLS").size(10.0).color(muted(ui)));
                        ui.add_space(8.0);
                    }
                    let selected = self.page == *page;
                    let text = RichText::new(page.name()).size(13.0).color(if selected {
                        accent(ui)
                    } else {
                        if ui.visuals().dark_mode {
                            Color32::from_rgb(190, 199, 210)
                        } else {
                            Color32::from_rgb(44, 60, 50)
                        }
                    });
                    let button = egui::Button::new(text)
                        .fill(if selected {
                            mint_surface(ui)
                        } else {
                            Color32::TRANSPARENT
                        })
                        .min_size(Vec2::new(170.0, 38.0))
                        .corner_radius(8);
                    if ui.add(button).clicked() {
                        self.page = *page;
                        self.search.clear();
                    }
                    ui.add_space(4.0);
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.label(
                        RichText::new("Native desktop · Development build")
                            .size(10.0)
                            .color(muted(ui)),
                    );
                    ui.add_space(10.0);
                    if ui
                        .checkbox(&mut self.config.expert, "Expert controls")
                        .changed()
                    {
                        let _ = self.config.save(&self.data);
                    }
                    ui.label(RichText::new("WORKSPACE").size(10.0).color(muted(ui)));
                });
            });
    }
    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(self.page.name()).size(28.0).strong());
                ui.label(
                    RichText::new(self.page.subtitle())
                        .size(13.0)
                        .color(muted(ui)),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(
                        !self.busy && self.config.tools_ready(),
                        egui::Button::new("Scan devices")
                            .fill(mint_surface(ui))
                            .min_size(Vec2::new(120.0, 34.0)),
                    )
                    .clicked()
                {
                    self.scan();
                }
                if self.busy {
                    ui.spinner();
                }
            });
        });
        ui.add_space(18.0);
        ui.separator();
        ui.add_space(16.0);
    }
    fn device_bar(&mut self, ui: &mut egui::Ui) {
        let old_selection = self.selected;
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("TARGET DEVICE").size(10.0).color(muted(ui)));
                ui.add_enabled_ui(!self.busy, |ui| {
                    egui::ComboBox::from_id_salt("target-device")
                        .width(300.0)
                        .selected_text(
                            self.selected
                                .and_then(|i| self.devices.get(i))
                                .map(|d| {
                                    format!(
                                        "{} · {}",
                                        self.config.device_label(&d.serial, &d.name()),
                                        d.mode.label()
                                    )
                                })
                                .unwrap_or_else(|| "No device selected".into()),
                        )
                        .show_ui(ui, |ui| {
                            for (i, d) in self.devices.iter().enumerate() {
                                ui.selectable_value(
                                    &mut self.selected,
                                    Some(i),
                                    format!(
                                        "{} · {} · …{}",
                                        self.config.device_label(&d.serial, &d.name()),
                                        d.mode.label(),
                                        d.serial
                                            .chars()
                                            .rev()
                                            .take(4)
                                            .collect::<String>()
                                            .chars()
                                            .rev()
                                            .collect::<String>()
                                    ),
                                );
                            }
                        });
                });
                if ui
                    .add_enabled(
                        self.selected.is_some() && !self.busy,
                        egui::Button::new("Inspect"),
                    )
                    .clicked()
                {
                    self.inspect();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mode = self
                        .device()
                        .map(|d| d.mode.label().to_owned())
                        .unwrap_or_else(|_| "DISCONNECTED".into());
                    ui.label(RichText::new(mode).size(11.0).color(accent(ui)));
                });
            });
            if ui
                .checkbox(
                    &mut self.include_disabled,
                    "Include disabled devices in next scan",
                )
                .changed()
            {
                self.status = "Scan again to apply device visibility".into();
            }
        });
        if old_selection != self.selected {
            self.clear_device_caches();
        }
        ui.add_space(16.0);
    }
    fn confirmation_modal(&mut self, ctx: &egui::Context) {
        let Some(pending) = &self.pending else {
            return;
        };
        let plan = pending.plan.clone();
        let mut open = true;
        let mut execute = false;
        egui::Window::new("Review operation").collapsible(false).resizable(true).default_width(760.0).open(&mut open).show(ctx,|ui|{
            ui.heading(&plan.title);
            ui.label(format!("Frozen operation: {}",plan.id()));
            if let Some(options)=plan.effective_options(){
                ui.label(format!("Wipe userdata: {} · Slots: {} · Final reboot: {}",options.wipe_userdata,options.slots.join(" / "),options.final_reboot));
                ui.label(format!("Disable verity: {} · Disable AVB verification: {} · Force: {}",options.disable_verity,options.disable_verification,options.force));
            }
            ui.label(format!("Target: {} / {}",plan.device.name(),plan.device.mode.label()));if plan.destructive{ui.colored_label(warning_color(ui),"This operation changes the selected device. Completed writes cannot be rolled back by cancelling.");}for w in &plan.warnings{ui.colored_label(warning_color(ui),w);}ui.add_space(10.0);
            egui::ScrollArea::vertical().id_salt("review-command-list").max_height(280.0).show(ui,|ui|{for(i,s)in plan.steps.iter().enumerate(){let line=match s{Step::Command(c)=>c.display(),Step::WaitFastboot=>"Wait for this device to enter fastboot".into(),Step::Sleep(s)=>format!("Wait {s} seconds")};ui.label(RichText::new(format!("{:02}  {line}",i+1)).monospace().size(11.0));}for a in &plan.artifacts{ui.label(RichText::new(format!("SHA256 {}  {}",&a.sha256[..16],a.path.file_name().unwrap_or_default().to_string_lossy())).size(10.0).color(muted(ui)));}});
            ui.separator();ui.checkbox(&mut self.acknowledge,"I reviewed the target, input files and commands");ui.label(format!("Type {} to confirm this device:",plan.device.confirmation()));ui.text_edit_singleline(&mut self.confirmation);
            ui.horizontal(|ui|{let ready=self.acknowledge&&plan.validate_confirmation(&self.confirmation).is_ok()&&!self.busy;execute=ui.add_enabled(ready,egui::Button::new("Execute reviewed plan").fill(mint_surface(ui))).clicked();});
        });
        if !open {
            self.pending = None;
        }
        if execute {
            self.run_pending();
        }
    }
}
impl PixelFlasher {
    fn draw(&mut self, ctx: &egui::Context) {
        if self.busy && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.error = Some("Stop at a checkpoint before closing the application".into());
        }
        let density = if self.config.compact {
            Vec2::new(7.0, 6.0)
        } else {
            Vec2::new(10.0, 10.0)
        };
        ctx.style_mut(|s| s.spacing.item_spacing = density);
        self.poll();
        self.process_dropped_file(ctx);
        if self.log.len() > 2000 {
            self.log.drain(..self.log.len() - 1500);
        }
        self.config.root_solution = self.solution.label().into();
        ctx.request_repaint_after(Duration::from_millis(if self.busy { 100 } else { 500 }));
        self.sidebar(ctx);
        egui::TopBottomPanel::bottom("activity")
            .resizable(false)
            .exact_height(if self.show_console { 130.0 } else { 44.0 })
            .min_height(42.0)
            .frame(
                egui::Frame::new()
                    .fill(if self.config.dark {
                        Color32::from_rgb(13, 18, 25)
                    } else {
                        Color32::from_rgb(235, 242, 237)
                    })
                    .inner_margin(egui::Margin::symmetric(24, 12)),
            )
            .show(ctx, |ui| {
                ui.set_min_height(ui.available_height());
                ui.horizontal(|ui| {
                    ui.label(RichText::new("ACTIVITY").size(10.0).color(muted(ui)));
                    ui.label(RichText::new(&self.status).size(11.0).color(accent(ui)));
                    if self.busy {
                        if let Some(started) = self.job_started {
                            ui.label(format!("{}s", started.elapsed().as_secs()));
                        }
                        if let Some(progress) = self.job_progress {
                            ui.add(egui::ProgressBar::new(progress).desired_width(120.0));
                        }
                        ui.spinner();
                        if ui.small_button("Cancel").clicked() {
                            self.cancel.store(true, Ordering::Relaxed);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.checkbox(&mut self.show_console, "Show log");
                        if ui.small_button("Export log").clicked()
                            && let Some(path) = rfd::FileDialog::new()
                                .set_file_name("pixelflasher-session.txt")
                                .save_file()
                        {
                            let mut text = self.log.join("\n");
                            for d in &self.devices {
                                text = text.replace(&d.serial, "[device]");
                            }
                            if let Err(e) = fs::write(path, text) {
                                self.error = Some(e.to_string());
                            }
                        }
                        if ui.small_button("Clear").clicked() {
                            self.log.clear();
                        }
                    });
                });
                if self.show_console {
                    egui::ScrollArea::vertical()
                        .id_salt("activity-log")
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for line in &self.log {
                                ui.label(RichText::new(line).monospace().size(11.0).color(
                                    if line.starts_with("ERROR") {
                                        Color32::from_rgb(255, 145, 145)
                                    } else {
                                        MUTED
                                    },
                                ));
                            }
                        });
                }
            });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(if self.config.dark {
                        Color32::from_rgb(20, 26, 34)
                    } else {
                        Color32::from_rgb(240, 246, 242)
                    })
                    .inner_margin(egui::Margin::symmetric(30, 24)),
            )
            .show(ctx, |ui| {
                self.header(ui);
                if let Some(error) = self.error.clone() {
                    card(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.colored_label(Color32::from_rgb(255, 157, 140), error);
                            if ui.small_button("Dismiss").clicked() {
                                self.error = None;
                            }
                        });
                    });
                    ui.add_space(12.0);
                }
                egui::ScrollArea::vertical()
                    .id_salt(("main-content", self.page.name()))
                    .show(ui, |ui| {
                        if self.page != Page::Settings {
                            self.device_bar(ui);
                        }
                        match self.page {
                            Page::Overview => self.overview(ui),
                            Page::Firmware => self.firmware_page(ui),
                            Page::Flash => self.flash_page(ui),
                            Page::Apps => self.apps_page(ui),
                            Page::Root => self.root_page(ui),
                            Page::Backups => self.backups_page(ui),
                            Page::Wireless => self.wireless_page(ui),
                            Page::Utilities => self.utilities_page(ui),
                            Page::Settings => self.settings_page(ui),
                        }
                    });
            });
        self.confirmation_modal(ctx);
    }
}
impl eframe::App for PixelFlasher {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.draw(ctx);
    }
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        self.cancel.store(true, Ordering::Relaxed);
        self.config.root_solution = self.solution.label().into();
        let _ = self.config.save(&self.data);
    }
}

fn theme(ctx: &egui::Context, dark: bool) {
    ctx.set_theme(if dark {
        egui::ThemePreference::Dark
    } else {
        egui::ThemePreference::Light
    });
    let mut style = (*ctx.style()).clone();
    style.visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    if dark {
        style.visuals.override_text_color = Some(Color32::from_rgb(221, 230, 237));
        style.visuals.widgets.noninteractive.fg_stroke.color = Color32::from_rgb(221, 230, 237);
        style.visuals.widgets.inactive.fg_stroke.color = Color32::from_rgb(221, 230, 237);
        style.visuals.widgets.hovered.fg_stroke.color = Color32::WHITE;
        style.visuals.extreme_bg_color = Color32::from_rgb(16, 23, 31);
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(37, 49, 62);
        style.spacing.item_spacing = Vec2::new(10.0, 10.0);
        style.spacing.button_padding = Vec2::new(12.0, 8.0);
        style.visuals.selection.bg_fill = Color32::from_rgb(36, 77, 65);
        style.visuals.selection.stroke = egui::Stroke::new(1.0_f32, ACCENT);
        style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, BORDER);
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(37, 49, 62);
        style.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(7);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(48, 63, 71);
        style.visuals.widgets.active.bg_fill = Color32::from_rgb(46, 81, 69);
    } else {
        style.visuals.override_text_color = Some(Color32::from_rgb(30, 44, 43));
        style.visuals.hyperlink_color = Color32::from_rgb(20, 99, 146);
        style.visuals.selection.bg_fill = Color32::from_rgb(183, 231, 211);
        style.visuals.selection.stroke = egui::Stroke::new(1.0_f32, Color32::from_rgb(24, 94, 71));
        style.spacing.item_spacing = Vec2::new(10.0, 10.0);
        style.spacing.button_padding = Vec2::new(12.0, 8.0);
    }
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(13.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
    ctx.set_style(style);
}
fn warning_color(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(246, 197, 113)
    } else {
        Color32::from_rgb(142, 81, 0)
    }
}
fn muted(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        MUTED
    } else {
        Color32::from_rgb(78, 96, 87)
    }
}
fn accent(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        ACCENT
    } else {
        Color32::from_rgb(20, 103, 75)
    }
}
fn mint_surface(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(34, 65, 57)
    } else {
        Color32::from_rgb(187, 225, 207)
    }
}
fn card<R>(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(if ui.visuals().dark_mode {
            CARD
        } else {
            Color32::from_rgb(245, 248, 246)
        })
        .stroke(egui::Stroke::new(1.0_f32, BORDER))
        .corner_radius(12)
        .inner_margin(18)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            content(ui)
        })
        .inner
}
fn field(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(RichText::new(label).size(10.0).color(muted(ui)));
    ui.label(RichText::new(value).size(15.0).strong());
}
fn path_picker(ui: &mut egui::Ui, value: &mut String, label: &str, extensions: &[&str]) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(label);
        changed |= ui
            .add(egui::TextEdit::singleline(value).desired_width(420.0))
            .changed();
        if ui.button("Browse").clicked() {
            let dialog = rfd::FileDialog::new().add_filter(label, extensions);
            if let Some(p) = dialog.pick_file() {
                *value = p.display().to_string();
                changed = true;
            }
        }
    });
    changed
}

#[cfg(test)]
mod workflow_tests {
    use super::*;
    fn app() -> (tempfile::TempDir, PixelFlasher) {
        let directory = tempfile::tempdir().unwrap();
        let app =
            PixelFlasher::from_context(&egui::Context::default(), directory.path().to_owned());
        (directory, app)
    }
    #[test]
    fn target_change_discards_review_and_sensitive_drafts() {
        let (_dir, mut app) = app();
        let d = Device {
            serial: "SCOPE_FIXTURE".into(),
            ..Default::default()
        };
        let mut p = Plan::new("scope", &d);
        p.add(Invocation::adb(&d.serial, &["shell", "true"], "fixture"));
        app.prepare(Ok(p));
        app.file_content = "secret draft".into();
        app.superkey = "private key".into();
        app.options.wipe = true;
        app.confirmation = d.confirmation();
        app.acknowledge = true;
        app.clear_device_caches();
        assert!(app.pending.is_none());
        assert!(
            app.file_content.is_empty() && app.superkey.is_empty() && app.confirmation.is_empty()
        );
        assert!(!app.options.wipe && !app.acknowledge);
    }
    #[test]
    fn configuration_result_cannot_cross_device_or_path() {
        let (_dir, mut app) = app();
        app.devices.push(Device {
            serial: "CURRENT_DEVICE".into(),
            ..Default::default()
        });
        app.selected = Some(0);
        app.file_content = "keep current draft".into();
        for (serial, path) in [
            ("OLD_DEVICE", app.file_path.clone()),
            ("CURRENT_DEVICE", "/data/adb/other.json".into()),
        ] {
            app.tx
                .send(Event::Result(Ok(Payload::FileText {
                    serial: serial.into(),
                    path,
                    text: "stale contents".into(),
                })))
                .unwrap();
            app.poll();
            assert_eq!(app.file_content, "keep current draft");
            assert!(app.error.is_some());
        }
    }
    #[test]
    fn all_workspaces_render_all_states_at_minimum_size_in_both_themes() {
        for dark in [true, false] {
            for page in Page::ALL {
                for state in ["empty", "populated", "busy", "failed", "expert"] {
                    let (directory, mut app) = app();
                    let ctx = egui::Context::default();
                    theme(&ctx, dark);
                    app.config.dark = dark;
                    app.page = page;
                    if state != "empty" {
                        app.devices.push(Device {
                            serial: "UI_FIXTURE".into(),
                            product: "komodo".into(),
                            model: "Fixture - not connected".into(),
                            slot: "a".into(),
                            ..Default::default()
                        });
                        app.selected = Some(0);
                    }
                    if state == "busy" {
                        app.busy = true;
                        app.job_started = Some(Instant::now());
                        app.job_progress = Some(0.5);
                    }
                    if state == "failed" {
                        app.error = Some("Fixture operation failed; review required".into());
                    }
                    app.config.expert = state == "expert";
                    let output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                Vec2::new(1024.0, 720.0),
                            )),
                            ..Default::default()
                        },
                        |ctx| app.draw(ctx),
                    );
                    assert!(!output.shapes.is_empty(), "{} {state}", page.name());
                    for shape in &output.shapes {
                        if let egui::Shape::Text(text) = &shape.shape {
                            assert!(
                                !text.galley.text().contains("used twice"),
                                "{} {state}",
                                page.name()
                            );
                        }
                    }
                    assert!(directory.path().exists());
                }
            }
        }
    }
    #[test]
    fn diagnostic_results_never_overwrite_root_configuration() {
        let (_directory, mut app) = app();
        app.file_content = "{\"keep\":true}".into();
        app.tx
            .send(Event::Result(Ok(Payload::InspectionText(
                "certificate report".into(),
            ))))
            .unwrap();
        app.poll();
        assert_eq!(app.file_content, "{\"keep\":true}");
        assert_eq!(app.utility_json, "certificate report");
        app.tx
            .send(Event::Result(Ok(Payload::Text("battery report".into()))))
            .unwrap();
        app.poll();
        assert_eq!(app.file_content, "{\"keep\":true}");
        app.devices.push(Device {
            serial: "CONFIG_FIXTURE".into(),
            ..Default::default()
        });
        app.selected = Some(0);
        app.tx
            .send(Event::Result(Ok(Payload::FileText {
                serial: "CONFIG_FIXTURE".into(),
                path: app.file_path.clone(),
                text: "{\"loaded\":true}".into(),
            })))
            .unwrap();
        app.poll();
        assert_eq!(app.file_content, "{\"loaded\":true}");
    }
    #[test]
    fn scan_remembers_target_and_removes_stale_package_selection() {
        let (_directory, mut app) = app();
        let a = Device {
            serial: "FIXTURE_A".into(),
            ..Default::default()
        };
        let b = Device {
            serial: "FIXTURE_B".into(),
            ..Default::default()
        };
        app.devices = vec![a.clone(), b.clone()];
        app.selected = Some(1);
        app.selected_packages.insert("old.app".into());
        app.packages.push(Package {
            name: "old.app".into(),
            ..Default::default()
        });
        app.tx
            .send(Event::Result(Ok(Payload::Devices(vec![b, a]))))
            .unwrap();
        app.poll();
        assert_eq!(app.selected, Some(0));
        assert_eq!(app.device().unwrap().serial, "FIXTURE_B");
        assert!(app.packages.is_empty());
        assert!(app.selected_packages.is_empty());
    }
    #[test]
    fn saved_disabled_device_is_hidden_until_requested() {
        let (_directory, mut app) = app();
        app.config
            .devices
            .push(pixelflasher_rust::config::SavedDevice {
                serial: "HIDDEN_FIXTURE".into(),
                label: "Hidden".into(),
                enabled: false,
            });
        let device = Device {
            serial: "HIDDEN_FIXTURE".into(),
            ..Default::default()
        };
        app.tx
            .send(Event::Result(Ok(Payload::Devices(vec![device.clone()]))))
            .unwrap();
        app.poll();
        assert!(app.devices.is_empty());
        app.include_disabled = true;
        app.tx
            .send(Event::Result(Ok(Payload::Devices(vec![device]))))
            .unwrap();
        app.poll();
        assert_eq!(app.devices.len(), 1);
    }
    #[test]
    fn stored_root_solution_survives_restart() {
        let directory = tempfile::tempdir().unwrap();
        let config = Config {
            root_solution: "Magisk".into(),
            ..Default::default()
        };
        config.save(directory.path()).unwrap();
        let app =
            PixelFlasher::from_context(&egui::Context::default(), directory.path().to_owned());
        assert_eq!(app.solution, Solution::Magisk);
        assert!(!app.show_console);
    }
    #[test]
    fn background_plan_preparation_only_opens_review() {
        let (_directory, mut app) = app();
        app.config.platform_tools = PathBuf::from("missing-fixture-tools");
        let device = Device {
            serial: "UNCONNECTED_TEST_DEVICE".into(),
            product: "fixture".into(),
            ..Default::default()
        };
        app.prepare_async("Preparing fixture plan", move || {
            let mut plan = Plan::new("fixture review", &device);
            plan.add(Invocation::adb(
                &device.serial,
                &["reboot"],
                "fixture command never executed",
            ));
            Ok(Pending {
                plan: OperationPlan::freeze(plan)?,
                patch: None,
                backup: None,
            })
        });
        let deadline = Instant::now() + Duration::from_secs(2);
        while app.busy && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
            app.poll();
        }
        assert!(!app.busy);
        assert!(app.pending.is_some());
        assert!(!app.acknowledge);
        assert!(app.confirmation.is_empty());
        assert!(
            !app.log
                .iter()
                .any(|line| line.contains("fixture command never executed"))
        );
    }
    #[test]
    fn native_pages_do_not_paint_widget_id_collision_warnings() {
        fn warning(shape: &egui::Shape) -> bool {
            match shape {
                egui::Shape::Text(text) => {
                    let text = text.galley.text();
                    text.contains("use of ScrollArea") || text.contains("used twice")
                }
                egui::Shape::Vec(shapes) => shapes.iter().any(warning),
                _ => false,
            }
        }
        for page in Page::ALL {
            let directory = tempfile::tempdir().unwrap();
            let ctx = egui::Context::default();
            let mut app = PixelFlasher::from_context(&ctx, directory.path().to_owned());
            app.page = page;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1340.0, 900.0),
                )),
                ..Default::default()
            };
            let output = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| match page {
                    Page::Overview => app.overview(ui),
                    Page::Firmware => app.firmware_page(ui),
                    Page::Flash => app.flash_page(ui),
                    Page::Apps => app.apps_page(ui),
                    Page::Root => app.root_page(ui),
                    Page::Backups => app.backups_page(ui),
                    Page::Wireless => app.wireless_page(ui),
                    Page::Utilities => app.utilities_page(ui),
                    Page::Settings => app.settings_page(ui),
                });
            });
            assert!(
                !output.shapes.iter().any(|s| warning(&s.shape)),
                "{}",
                page.name()
            );
        }
    }
}
