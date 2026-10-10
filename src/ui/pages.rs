use super::*;

impl PixelFlasher {
    pub(super) fn overview(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) =
                    ui.allocate_exact_size(Vec2::new(100.0, 155.0), egui::Sense::hover());
                let phone = rect.shrink2(Vec2::new(18.0, 4.0));
                ui.painter()
                    .rect_filled(phone, 18, Color32::from_rgb(36, 57, 61));
                ui.painter().rect_stroke(
                    phone,
                    18,
                    egui::Stroke::new(2.0_f32, ACCENT),
                    egui::StrokeKind::Inside,
                );
                ui.painter().circle_filled(
                    egui::pos2(phone.center().x, phone.top() + 12.0),
                    3.0,
                    ACCENT,
                );
                ui.painter().rect_filled(
                    egui::Rect::from_center_size(
                        egui::pos2(phone.center().x, phone.bottom() - 12.0),
                        Vec2::new(30.0, 3.0),
                    ),
                    2,
                    ACCENT,
                );
                ui.add_space(18.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("A CLEARER PATH TO YOUR NEXT UPDATE")
                            .size(10.0)
                            .color(accent(ui)),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(
                            self.device()
                                .map(|d| d.name())
                                .unwrap_or_else(|_| "Your Pixel. Your control.".into()),
                        )
                        .size(29.0)
                        .strong(),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(
                            "Inspect the device, prepare your images, and review every operation.",
                        )
                        .color(muted(ui)),
                    );
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        if ui.button("Prepare firmware").clicked() {
                            self.page = Page::Firmware;
                        }
                        if ui.button("Open flash workspace").clicked() {
                            self.page = Page::Flash;
                        }
                    });
                });
            });
        });
        ui.add_space(18.0);
        ui.columns(3, |columns| {
            card(&mut columns[0], |ui| {
                field(
                    ui,
                    "DEVICE CONNECTION",
                    self.device()
                        .map(|d| d.mode.label().to_owned())
                        .unwrap_or_else(|_| "Not connected".into())
                        .as_str(),
                )
            });
            card(&mut columns[1], |ui| {
                field(
                    ui,
                    "IMAGE LIBRARY",
                    &format!("{} images", self.images.len()),
                )
            });
            card(&mut columns[2], |ui| {
                field(
                    ui,
                    "PLATFORM TOOLS",
                    if self.config.tools_ready() {
                        "Folder selected"
                    } else {
                        "Setup required"
                    },
                )
            });
        });
        ui.add_space(18.0);
        self.readiness(ui);
        ui.add_space(12.0);
        self.journal_panel(ui);
        ui.add_space(18.0);
        if let Ok(d) = self.device() {
            card(ui, |ui| {
                ui.heading("Device details");
                ui.add_space(10.0);
                egui::Grid::new("device-summary")
                    .num_columns(4)
                    .spacing([28.0, 16.0])
                    .show(ui, |ui| {
                        field(ui, "PRODUCT", &d.product);
                        field(
                            ui,
                            "ACTIVE SLOT",
                            if d.slot.is_empty() {
                                "Unknown"
                            } else {
                                &d.slot
                            },
                        );
                        field(
                            ui,
                            "BOOTLOADER",
                            match d.unlocked {
                                Some(true) => "Unlocked",
                                Some(false) => "Locked",
                                None => "Unknown",
                            },
                        );
                        field(
                            ui,
                            "ROOT",
                            if d.rooted {
                                &d.root_solution
                            } else {
                                "Not detected"
                            },
                        );
                        ui.end_row();
                        field(ui, "ANDROID", &d.detail("ro.build.version.release"));
                        field(ui, "API LEVEL", &d.detail("ro.build.version.sdk"));
                        field(ui, "ARCHITECTURE", &d.detail("ro.product.cpu.abi"));
                        field(
                            ui,
                            "SECURITY PATCH",
                            &d.detail("ro.build.version.security_patch"),
                        );
                        ui.end_row();
                    });
                ui.label(
                    RichText::new(&d.build)
                        .monospace()
                        .size(10.0)
                        .color(muted(ui)),
                );
            });
            ui.add_space(18.0);
            card(ui, |ui| {
                ui.heading("Device actions");
                ui.horizontal_wrapped(|ui| {
                    for (label, mode) in [
                        ("System", ""),
                        ("Bootloader", "bootloader"),
                        ("Recovery", "recovery"),
                        ("Fastbootd", "fastboot"),
                        ("Sideload", "sideload"),
                    ] {
                        if ui
                            .add_enabled(!self.busy, egui::Button::new(label))
                            .clicked()
                        {
                            let args = if mode.is_empty() {
                                vec!["reboot"]
                            } else {
                                vec!["reboot", mode]
                            };
                            self.prepare(flash::simple(
                                &d,
                                &format!("Reboot to {label}"),
                                &args,
                                false,
                                false,
                            ));
                        }
                    }
                    if ui.button("Refresh properties").clicked() {
                        self.inspect();
                    }
                });
                if self.config.expert {
                    ui.separator();
                    ui.horizontal(|ui|{if ui.button("Unlock bootloader").clicked(){let mut p=Plan::new("Unlock bootloader",&d);p.destructive=true;p.warnings.push("Unlocking erases all user data and requires on-device confirmation.".into());let result=p.prepare_fastboot().map(|_|{p.add(Invocation::fastboot(&d.serial,&["flashing","unlock"],"Unlock bootloader"));p});self.prepare(result);}
if ui.button("Lock bootloader").clicked(){let mut p=Plan::new("Lock bootloader",&d);p.destructive=true;p.warnings.push("Lock only with matching, verified stock firmware. Relocking erases user data and may make modified firmware unbootable.".into());let result=p.prepare_fastboot().map(|_|{p.add(Invocation::fastboot(&d.serial,&["flashing","lock"],"Lock bootloader"));p});self.prepare(result);}for slot in ["a","b"]{if ui.button(format!("Set slot {slot}")).clicked(){let mut p=Plan::new("Change active slot",&d);p.destructive=true;p.requires_unlocked=true;let result=p.prepare_fastboot().map(|_|{p.add(Invocation::fastboot(&d.serial,&["set_active",slot],"Change slot"));p});self.prepare(result);}}});
                }
            });
        } else {
            ui.label(RichText::new("No phone needed to prepare firmware. Drop a ZIP or image onto this window to get started.").color(muted(ui)));
        }
    }
    pub(super) fn firmware_page(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.heading("Local firmware");
            path_picker(
                ui,
                &mut self.firmware_path,
                "Firmware",
                &["zip", "img", "bin", "tgz", "tar", "gz"],
            );
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !self.busy && !self.firmware_path.is_empty(),
                        egui::Button::new("Process & verify").fill(mint_surface(ui)),
                    )
                    .clicked()
                {
                    let path = PathBuf::from(&self.firmware_path);
                    let data = self.data.clone();
                    self.config.last_firmware = path.clone();
                    self.run("Processing firmware", move |_, cancel, tx| {
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
                if let Some(f) = &self.firmware {
                    ui.label(
                        RichText::new(format!(
                            "{} · {} · {} extracted images",
                            f.kind,
                            f.product,
                            f.images.len()
                        ))
                        .color(accent(ui)),
                    );
                }
            });
        });
        ui.add_space(16.0);
        self.image_library(ui);
        ui.add_space(16.0);
        card(ui, |ui| {
            ui.heading("Official firmware & root releases");
            ui.horizontal_wrapped(|ui| {
                if ui
                    .checkbox(
                        &mut self.config.google_terms_accepted,
                        "I have read and agree to Google's firmware terms",
                    )
                    .changed()
                {
                    let _ = self.config.save(&self.data);
                }
                ui.hyperlink_to(
                    "Read factory image terms",
                    "https://developers.google.com/android/images#terms-and-conditions",
                );
                ui.hyperlink_to(
                    "Read OTA terms",
                    "https://developers.google.com/android/ota#terms-and-conditions",
                );
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.ota_catalog, "Full OTA packages");
                if ui
                    .add_enabled(
                        !self.busy && self.config.google_terms_accepted,
                        egui::Button::new("Load Google catalog"),
                    )
                    .clicked()
                {
                    let ota = self.ota_catalog;
                    let accepted = self.config.google_terms_accepted;
                    self.run("Loading firmware catalog", move |_, _, _| {
                        Ok(Payload::Downloads(services::firmware_catalog(
                            ota, accepted,
                        )?))
                    });
                }
                if ui
                    .add_enabled(!self.busy, egui::Button::new("Load root manager releases"))
                    .clicked()
                {
                    let repo = self.solution.repo().to_owned();
                    self.run("Loading root releases", move |_, _, _| {
                        Ok(Payload::Downloads(services::releases(&repo)?))
                    });
                }
            });
            ui.add(
                egui::TextEdit::singleline(&mut self.download_filter)
                    .hint_text("Filter by product, build or asset name")
                    .desired_width(f32::INFINITY),
            );
            let items = self.downloads.clone();
            let download_filter = self.download_filter.to_lowercase();
            egui::ScrollArea::vertical()
                .id_salt("firmware-downloads")
                .max_height(220.0)
                .show(ui, |ui| {
                    for item in items
                        .iter()
                        .filter(|i| i.name.to_lowercase().contains(&download_filter))
                    {
                        ui.horizontal(|ui| {
                            ui.label(&item.name);
                            if ui
                                .add_enabled(!self.busy, egui::Button::new("Download"))
                                .clicked()
                            {
                                let name = item.url.rsplit('/').next().unwrap_or("download.zip");
                                if let Some(path) =
                                    rfd::FileDialog::new().set_file_name(name).save_file()
                                {
                                    let item = item.clone();
                                    self.run("Downloading", move |_, cancel, tx| {
                                        services::download(&item, &path, &cancel, &|line| {
                                            let _ = tx.send(Event::Log(line));
                                        })?;
                                        Ok(Payload::Finished)
                                    });
                                }
                            }
                        });
                    }
                });
        });
    }
    fn image_library(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Boot image library");
                if ui.small_button("Import image").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("Boot image", &["img"])
                        .pick_file()
                {
                    let data = self.data.clone();
                    self.run("Importing image", move |_, _, _| {
                        let info = image::inspect(&path)?;
                        Library::open(&data)?.insert(&path, "Manual import", "", "", "", &info)?;
                        Ok(Payload::LibraryChanged(
                            "Image imported into the library.".into(),
                        ))
                    });
                }
                if ui.small_button("Refresh").clicked() {
                    self.refresh_library();
                }
            });
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.image_filter)
                        .hint_text("Search image, device or root solution")
                        .desired_width(ui.available_width() - 170.0),
                );
                ui.checkbox(&mut self.only_patched, "Patched only");
            });
            let filter = self.image_filter.to_lowercase();
            let images = self
                .images
                .iter()
                .filter(|r| {
                    (!self.only_patched || !r.patched_with.is_empty())
                        && format!(
                            "{} {} {} {}",
                            r.info.name, r.info.kind, r.product, r.patched_with
                        )
                        .to_lowercase()
                        .contains(&filter)
                })
                .cloned()
                .collect::<Vec<_>>();
            if images.is_empty() {
                ui.label(
                    RichText::new("Processed stock and patched images appear here.")
                        .color(muted(ui)),
                );
            }
            egui::ScrollArea::vertical()
                .id_salt("boot-image-library")
                .max_height(230.0)
                .show(ui, |ui| {
                    for record in &images {
                        let selected = self.image == Some(record.id);
                        if ui
                            .selectable_label(
                                selected,
                                format!(
                                    "{}   ·   {}   ·   {:.1} MiB   ·   {}",
                                    record.info.kind,
                                    if record.patched_with.is_empty() {
                                        "Stock / imported"
                                    } else {
                                        &record.patched_with
                                    },
                                    record.info.size as f64 / 1048576.0,
                                    record.product
                                ),
                            )
                            .clicked()
                        {
                            self.image = Some(record.id);
                            self.partition = record.info.kind.clone();
                        }
                        if selected {
                            if let Some(lineage) = &record.lineage {
                                ui.label(format!(
                                    "{} {} · {}",
                                    lineage.provider.label(),
                                    lineage.version,
                                    lineage.method
                                ));
                                ui.label(
                                    RichText::new(format!("Stock SHA256 {}", lineage.stock_sha256))
                                        .monospace()
                                        .size(10.0),
                                );
                            }
                            ui.label(
                                RichText::new(record.path.display().to_string())
                                    .size(10.0)
                                    .color(muted(ui)),
                            );
                            ui.label(
                                RichText::new(format!("SHA256 {}", record.info.sha256))
                                    .monospace()
                                    .size(10.0),
                            );
                            ui.horizontal(|ui| {
                                if ui.small_button("Inspect metadata").clicked() {
                                    self.utility_json = serde_json::to_string_pretty(&record.info)
                                        .unwrap_or_default();
                                    self.page = Page::Utilities;
                                }
                                if ui.small_button("Export copy").clicked()
                                    && let Some(dest) = rfd::FileDialog::new()
                                        .set_file_name(&record.info.name)
                                        .save_file()
                                {
                                    if dest.exists() {
                                        self.error = Some(
                                            "Destination exists; choose a new filename".into(),
                                        );
                                    } else if let Err(e) = fs::copy(&record.path, dest) {
                                        self.error = Some(e.to_string());
                                    }
                                }
                                if ui.small_button("Remove from library").clicked() {
                                    if let Err(e) =
                                        Library::open(&self.data).and_then(|l| l.remove(record.id))
                                    {
                                        self.error = Some(e.to_string());
                                    }
                                    self.image = None;
                                    self.refresh_library();
                                }
                            });
                        }
                    }
                });
        });
    }
    pub(super) fn flash_page(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.update_tab, 0, "Factory / OTA update");
            ui.selectable_value(&mut self.update_tab, 1, "Image operations");
        });
        ui.add_space(12.0);
        card(ui, |ui| {
            ui.heading("Flash settings");
            ui.label(
                RichText::new("Device and input hashes are checked again before execution.")
                    .color(muted(ui)),
            );
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("slot")
                    .selected_text(match self.options.slot {
                        Slot::Current => "Current slot",
                        Slot::Inactive => "Inactive slot",
                        Slot::Both => "Both slots",
                    })
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(
                                matches!(self.options.slot, Slot::Current),
                                "Current slot",
                            )
                            .clicked()
                        {
                            self.options.slot = Slot::Current;
                        }
                        if ui
                            .selectable_label(
                                matches!(self.options.slot, Slot::Inactive),
                                "Inactive slot",
                            )
                            .clicked()
                        {
                            self.options.slot = Slot::Inactive;
                        }
                        if self.config.expert
                            && ui
                                .selectable_label(
                                    matches!(self.options.slot, Slot::Both),
                                    "Both slots",
                                )
                                .clicked()
                        {
                            self.options.slot = Slot::Both;
                        }
                    });
                ui.checkbox(&mut self.options.no_reboot, "Skip final reboot");
                if self.config.expert {
                    ui.checkbox(&mut self.options.wipe, "Wipe user data");
                }
            });
            if self.config.expert {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.options.disable_verity, "Disable verity");
                    ui.checkbox(
                        &mut self.options.disable_verification,
                        "Disable verification",
                    );
                    ui.checkbox(&mut self.options.force, "Force update");
                });
            }
        });
        ui.add_space(16.0);
        if self.update_tab == 0 {
            card(ui, |ui| {
                ui.heading("Factory firmware / full OTA");
                if let Some(f) = self.firmware.clone() {
                    ui.label(format!(
                        "{} · {}",
                        f.path.file_name().unwrap_or_default().to_string_lossy(),
                        f.kind
                    ));
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                !self.busy && f.kind == "Factory image",
                                egui::Button::new("Review factory flash"),
                            )
                            .clicked()
                        {
                            let device = self.device();
                            let firmware = f.clone();
                            let options = self.options.clone();
                            self.prepare_async("Preparing factory flash plan", move || {
                                Ok(Pending {
                                    plan: OperationPlan::freeze(flash::factory(
                                        &device?, &firmware, &options, None,
                                    )?)?,
                                    patch: None,
                                    backup: None,
                                })
                            });
                        }
                        if ui
                            .add_enabled(
                                !self.busy && f.kind == "Full OTA",
                                egui::Button::new("Review OTA sideload"),
                            )
                            .clicked()
                        {
                            let device = self.device();
                            let firmware = f.clone();
                            self.prepare_async("Preparing OTA sideload plan", move || {
                                Ok(Pending {
                                    plan: OperationPlan::freeze(flash::sideload(
                                        &device?,
                                        &firmware.path,
                                    )?)?,
                                    patch: None,
                                    backup: None,
                                })
                            });
                        }
                        if self.image.is_some()
                            && ui
                                .add_enabled(
                                    !self.busy && f.kind == "Factory image",
                                    egui::Button::new("Factory + selected root image"),
                                )
                                .clicked()
                        {
                            let device = self.device();
                            let boot = self.selected_image();
                            let firmware = f.clone();
                            let options = self.options.clone();
                            self.prepare_async(
                                "Preparing firmware and root-image plan",
                                move || {
                                    let boot = boot?;
                                    Ok(Pending {
                                        plan: OperationPlan::freeze(flash::factory(
                                            &device?,
                                            &firmware,
                                            &options,
                                            Some((&boot.path, &boot.info.kind)),
                                        )?)?,
                                        patch: None,
                                        backup: None,
                                    })
                                },
                            );
                        }
                    });
                    if f.kind == "Full OTA" {
                        ui.label(
                        RichText::new(
                            "Enter recovery > Apply update from ADB, rescan, then review sideload.",
                        )
                        .color(muted(ui)),
                    );
                    }
                } else {
                    ui.label("Process a firmware package in Firmware library first.");
                }
            });
            ui.add_space(16.0);
        }
        if self.update_tab == 1 {
            self.image_library(ui);
            ui.add_space(16.0);
            card(ui, |ui| {
                ui.heading("Selected image operation");
                ui.horizontal(|ui| {
                    ui.label("Target partition");
                    egui::ComboBox::from_id_salt("partition")
                        .selected_text(&self.partition)
                        .show_ui(ui, |ui| {
                            for p in [
                                "boot",
                                "init_boot",
                                "vendor_boot",
                                "vendor_kernel_boot",
                                "recovery",
                                "vbmeta",
                                "dtbo",
                            ] {
                                ui.selectable_value(&mut self.partition, p.into(), p);
                            }
                        });
                    ui.checkbox(&mut self.live, "Live boot (boot only)");
                    if ui
                        .add_enabled(
                            !self.busy && self.image.is_some(),
                            egui::Button::new("Review image operation").fill(mint_surface(ui)),
                        )
                        .clicked()
                    {
                        let device = self.device();
                        let boot = self.selected_image();
                        let partition = self.partition.clone();
                        let options = self.options.clone();
                        let live = self.live;
                        self.prepare_async("Preparing image operation", move || {
                            let d = device?;
                            let boot = boot?;
                            if !boot.product.is_empty() {
                                ensure!(
                                    boot.product == d.product,
                                    "Library image product does not match the selected device"
                                );
                            }
                            Ok(Pending {
                                plan: OperationPlan::freeze(flash::boot_image(
                                    &d, &boot.path, &partition, live, &options,
                                )?)?,
                                patch: None,
                                backup: None,
                            })
                        });
                    }
                });
            });
        }
    }
    pub(super) fn apps_page(&mut self, ui: &mut egui::Ui) {
        if self.config.expert {
            card(ui, |ui| {
                ui.heading("APK installer options");
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.install_store_owner, "Set Play Store as installer");
                    ui.checkbox(&mut self.install_bypass, "Bypass low target SDK block");
                });
            });
            ui.add_space(16.0);
        }
        self.app_privileges(ui);
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Installed applications");
                if ui
                    .add_enabled(!self.busy, egui::Button::new("Load apps"))
                    .clicked()
                {
                    if let Ok(d) = self.device() {
                        self.run("Reading applications", move |t, _, _| {
                            Ok(Payload::Apps(services::packages(&t, &d)?))
                        });
                    } else {
                        self.error = Some("Select a device first".into());
                    }
                }
                if ui.button("Install APK").clicked()
                    && let (Some(path), Ok(d)) = (
                        rfd::FileDialog::new()
                            .add_filter("Android package", &["apk"])
                            .pick_file(),
                        self.device(),
                    )
                {
                    self.prepare(services::install_apk(
                        &d,
                        &path,
                        self.install_store_owner,
                        self.install_bypass,
                    ));
                }
            });
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Filter package names")
                    .desired_width(f32::INFINITY),
            );
            ui.horizontal_wrapped(|ui| {
                for action in [
                    "launch",
                    "enable",
                    "disable",
                    "uninstall",
                    "clear",
                    "kill",
                    "details",
                    "permissions",
                ] {
                    if ui
                        .add_enabled(
                            !self.busy && !self.selected_packages.is_empty(),
                            egui::Button::new(action),
                        )
                        .clicked()
                    {
                        self.prepare(self.device().and_then(|d| {
                            services::package_action(
                                &d,
                                &self.selected_packages.iter().cloned().collect::<Vec<_>>(),
                                action,
                            )
                        }));
                    }
                }
                if ui.button("Export list").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .set_file_name("packages.csv")
                        .save_file()
                {
                    let text = format!(
                        "package,uid,system,disabled\n{}",
                        self.packages
                            .iter()
                            .map(|p| format!("{},{},{},{}", p.name, p.uid, p.system, p.disabled))
                            .collect::<Vec<_>>()
                            .join("\n")
                    );
                    if let Err(e) = fs::write(path, text) {
                        self.error = Some(e.to_string());
                    }
                }
                if ui
                    .add_enabled(
                        self.selected_packages.len() == 1 && !self.busy,
                        egui::Button::new("Download APK"),
                    )
                    .clicked()
                    && let (Some(directory), Ok(d), Some(name)) = (
                        rfd::FileDialog::new().pick_folder(),
                        self.device(),
                        self.selected_packages.iter().next().cloned(),
                    )
                {
                    self.run("Downloading installed APK", move |t, c, tx| {
                        services::pull_apk(&t, &d, &name, &directory, &c, &move |s| {
                            let _ = tx.send(Event::Log(s));
                        })?;
                        Ok(Payload::Finished)
                    });
                }
            });
            ui.label(
                RichText::new(format!(
                    "{} applications · {} selected",
                    self.packages.len(),
                    self.selected_packages.len()
                ))
                .color(muted(ui)),
            );
            egui::ScrollArea::vertical()
                .id_salt("installed-app-list")
                .max_height(380.0)
                .show(ui, |ui| {
                    for package in self
                        .packages
                        .iter()
                        .filter(|p| p.name.to_lowercase().contains(&self.search.to_lowercase()))
                    {
                        ui.horizontal(|ui| {
                            let mut selected = self.selected_packages.contains(&package.name);
                            if ui.checkbox(&mut selected, &package.name).changed() {
                                if selected {
                                    self.selected_packages.insert(package.name.clone());
                                } else {
                                    self.selected_packages.remove(&package.name);
                                }
                            }
                            if package.system {
                                ui.label(RichText::new("SYSTEM").size(9.0).color(muted(ui)));
                            }
                            if package.disabled {
                                ui.label(
                                    RichText::new("DISABLED")
                                        .size(9.0)
                                        .color(Color32::from_rgb(255, 189, 108)),
                                );
                            }
                        });
                    }
                });
        });
    }
    pub(super) fn root_page(&mut self, ui: &mut egui::Ui) {
        if !self.root_studio_header(ui) {
            return;
        }
        if self.root_tab == 0 {
            card(ui, |ui| {
                ui.heading("Manager assets");
                path_picker(ui, &mut self.root_apk, "Manager APK", &["apk"]);
                ui.horizontal(|ui| {
                    if ui.button("Install manager").clicked() {
                        self.prepare(self.device().and_then(|d| {
                            providers::provider(self.solution)
                                .manager_install_plan(&d, &PathBuf::from(&self.root_apk))
                        }));
                    }
                    if ui.button("Browse releases").clicked() {
                        self.page = Page::Firmware;
                        let repo = self.solution.repo().to_owned();
                        self.run("Loading root releases", move |_, _, _| {
                            Ok(Payload::Downloads(services::releases(&repo)?))
                        });
                    }
                });
                ui.label(
                RichText::new(
                    "Patch assets are extracted from your selected manager APK for the device ABI.",
                )
                .color(muted(ui)),
            );
            });
            ui.add_space(16.0);
            self.image_library(ui);
            ui.add_space(16.0);
            card(ui, |ui| {
                ui.heading("Create patched image");
                if self.config.expert {
                    path_picker(
                        ui,
                        &mut self.kernel_image,
                        "Optional GKI kernel",
                        &["img", "gz", "bin"],
                    );
                    if ui
                        .add_enabled(
                            !self.busy && !self.kernel_image.is_empty(),
                            egui::Button::new("Review GKI kernel replacement"),
                        )
                        .clicked()
                    {
                        let device = self.device();
                        let image = self.selected_image();
                        let kernel = PathBuf::from(&self.kernel_image);
                        let apk = PathBuf::from(&self.root_apk);
                        let solution = self.solution;
                        let data = self.data.clone();
                        self.prepare_async("Preparing kernel patch plan", move || {
                            let job = pixelflasher_rust::root_kernel::patch(
                                &device?,
                                &image?.path,
                                &kernel,
                                &apk,
                                solution,
                                &data,
                            )?;
                            Ok(Pending {
                                plan: OperationPlan::freeze(job.plan)?,
                                backup: None,
                                patch: Some(PatchResult {
                                    stock: job.stock,
                                    result: job.result,
                                    solution: job.solution,
                                    provider_version: job.provider_version,
                                    method: job.method,
                                }),
                            })
                        });
                    }
                }

                ui.horizontal(|ui| {
                    ui.label("KMI override (optional)");
                    ui.text_edit_singleline(&mut self.kmi);
                });
                if self.solution == Solution::APatch {
                    ui.horizontal(|ui| {
                        ui.label("APatch superkey");
                        ui.add(egui::TextEdit::singleline(&mut self.superkey).password(true));
                    });
                }
                if ui
                    .add_enabled(
                        !self.busy && self.image.is_some() && !self.root_apk.is_empty(),
                        egui::Button::new("Review patch operation").fill(mint_surface(ui)),
                    )
                    .clicked()
                {
                    let device = self.device();
                    let image = self.selected_image();
                    let apk = PathBuf::from(&self.root_apk);
                    let solution = self.solution;
                    let method = self.patch_method;
                    let kmi = self.kmi.clone();
                    let superkey = std::mem::take(&mut self.superkey);
                    let data = self.data.clone();
                    self.prepare_async("Preparing boot patch plan", move || {
                        let job = root::patch_with_options(
                            &device?,
                            &image?.path,
                            &apk,
                            &root::PatchOptions {
                                solution,
                                kmi: &kmi,
                                superkey: &superkey,
                                method,
                            },
                            &data,
                        )?;
                        Ok(Pending {
                            plan: OperationPlan::freeze(job.plan)?,
                            backup: None,
                            patch: Some(PatchResult {
                                stock: job.stock,
                                result: job.result,
                                solution: job.solution,
                                provider_version: job.provider_version,
                                method: job.method,
                            }),
                        })
                    });
                }
            });
            ui.add_space(16.0);
        }
        if self.root_tab == 1 {
            card(ui, |ui| {
                ui.horizontal(|ui|{ui.heading("Root modules");if ui.add_enabled(!self.busy,egui::Button::new("Refresh modules")).clicked()&& let Ok(d)=self.device(){self.run("Reading modules",move|t,_,_|Ok(Payload::Modules(root::parse_modules(&command::quiet(&t,&root::module_list(&d))?.stdout))));}
if ui.button("Install module ZIP").clicked()&& let(Some(path),Ok(d))=(rfd::FileDialog::new().add_filter("Root module",&["zip"]).pick_file(),self.device()){self.prepare(root::install_module(&d,&path,self.solution));}
if ui.button("Disable all modules").clicked()&& let Ok(d)=self.device(){self.prepare(flash::simple(&d,"Disable all root modules",&["shell","su","-c","for p in /data/adb/modules/*; do [ ! -f \"$p/module.prop\" ] || touch \"$p/disable\"; done"],true,false));}});
                let modules = self.modules.clone();
                for module in modules {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&module.name).strong());
                        ui.label(
                            RichText::new(format!("{} · {}", module.version, module.author))
                                .size(11.0)
                                .color(muted(ui)),
                        );
                        for action in [
                            if module.disabled { "enable" } else { "disable" },
                            if module.removed { "restore" } else { "remove" },
                            "action",
                        ] {
                            if ui.small_button(action).clicked() {
                                self.prepare(
                                    self.device()
                                        .and_then(|d| root::module_action(&d, &module.id, action)),
                                );
                            }
                        }
                    });
                    ui.label(
                        RichText::new(&module.description)
                            .size(11.0)
                            .color(muted(ui)),
                    );
                }
                if self.solution == Solution::Magisk {
                    ui.separator();
                    ui.label("Magisk settings");
                    ui.horizontal_wrapped(|ui| {
                        for (label, sql) in [
                            (
                                "Enable Zygisk",
                                "UPDATE settings SET value=1 WHERE key='zygisk';",
                            ),
                            (
                                "Disable Zygisk",
                                "UPDATE settings SET value=0 WHERE key='zygisk';",
                            ),
                        ] {
                            if ui.button(label).clicked()
                                && let Ok(d) = self.device()
                            {
                                self.prepare(flash::simple(
                                    &d,
                                    label,
                                    &[
                                        "shell",
                                        "su",
                                        "-c",
                                        &format!("magisk --sqlite {}", command::shell_quote(sql)),
                                    ],
                                    true,
                                    false,
                                ));
                            }
                        }
                        for (label, arg) in [
                            ("Enable denylist", "enable"),
                            ("Disable denylist", "disable"),
                        ] {
                            if ui.button(label).clicked()
                                && let Ok(d) = self.device()
                            {
                                self.prepare(flash::simple(
                                    &d,
                                    label,
                                    &["shell", "su", "-c", &format!("magisk --denylist {arg}")],
                                    true,
                                    false,
                                ));
                            }
                        }
                    });
                }
            });
        }
    }
    pub(super) fn backups_page(&mut self, ui: &mut egui::Ui) {
        self.verified_backup_panel(ui);
        if self.solution == Solution::Magisk {
            self.magisk_backup_page(ui);
        }
        card(ui, |ui| {
            ui.heading("Partition backups");
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!self.busy, egui::Button::new("List partitions"))
                    .clicked()
                    && let Ok(d) = self.device()
                {
                    self.run("Reading partitions", move |t, _, _| {
                        Ok(Payload::Partitions(services::partitions(&t, &d)?))
                    });
                }
                egui::ComboBox::from_id_salt("backup-partition")
                    .selected_text(if self.selected_partition.is_empty() {
                        "Select partition"
                    } else {
                        &self.selected_partition
                    })
                    .show_ui(ui, |ui| {
                        for p in &self.partitions {
                            ui.selectable_value(&mut self.selected_partition, p.clone(), p);
                        }
                    });
                if ui.button("Back up partition").clicked()
                    && let (Some(path), Ok(d)) = (
                        rfd::FileDialog::new()
                            .set_file_name(format!("{}.img", self.selected_partition))
                            .save_file(),
                        self.device(),
                    )
                {
                    self.prepare(services::backup_partition(
                        &d,
                        &self.selected_partition,
                        &path,
                    ));
                }
                if self.config.expert && ui.button("Erase partition").clicked() {
                    let name = self.selected_partition.clone();
                    let result = self.device().and_then(|d| {
                        command::validate_identifier(&name)?;
                        ensure!(
                            d.mode == Mode::Fastboot,
                            "Partition erasure requires fastboot mode"
                        );
                        flash::simple(&d, &format!("Erase {name}"), &["erase", &name], true, true)
                    });
                    self.prepare(result);
                }
            });
            ui.label(
                RichText::new("Dumping partitions requires a rooted device in Android mode.")
                    .color(muted(ui)),
            );
        });
        ui.add_space(16.0);
        card(ui, |ui| {
            ui.heading("Root data archive");
            ui.label("Back up /data/adb modules and root configuration to a local tar.gz file.");
            ui.horizontal(|ui| {
                if ui.button("Back up root data").clicked()
                    && let (Some(path), Ok(d)) = (
                        rfd::FileDialog::new()
                            .set_file_name("data-adb.tar.gz")
                            .save_file(),
                        self.device(),
                    )
                {
                    self.prepare(services::data_backup(&d, &path, false));
                }
                if ui.button("Restore root data").clicked()
                    && let (Some(path), Ok(d)) = (
                        rfd::FileDialog::new()
                            .add_filter("Root data archive", &["gz"])
                            .pick_file(),
                        self.device(),
                    )
                {
                    self.prepare(services::data_backup(&d, &path, true));
                }
                if ui.button("List Magisk backups").clicked()
                    && let Ok(d) = self.device()
                {
                    self.run("Reading Magisk backups", move |t, _, _| {
                        Ok(Payload::Backups(advanced::magisk_backups(&t, &d)?))
                    });
                }
            });
        });
    }
    pub(super) fn wireless_page(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.heading("Wireless debugging");
            ui.label("Enable Wireless debugging in Android Developer options. Pairing and connection use different ports.");
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.label("Address : port");
                ui.add(
                    egui::TextEdit::singleline(&mut self.endpoint)
                        .hint_text("192.168.1.20:37123")
                        .desired_width(300.0),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Pairing code");
                ui.add(
                    egui::TextEdit::singleline(&mut self.pair_code)
                        .password(true)
                        .desired_width(150.0),
                );
            });
            ui.horizontal(|ui| {
                for action in ["pair", "connect", "disconnect"] {
                    if ui
                        .add_enabled(!self.busy, egui::Button::new(action))
                        .clicked()
                    {
                        match services::wireless(action, &self.endpoint, &self.pair_code) {
                            Ok(call) => self.command(call),
                            Err(e) => self.error = Some(e.to_string()),
                        }
                    }
                }
            });
            ui.label(
                RichText::new("After connecting, scan devices to select the wireless target.")
                    .color(muted(ui)),
            );
        });
    }
    pub(super) fn utilities_page(&mut self, ui: &mut egui::Ui) {
        self.inspection_tools(ui);
        card(ui, |ui| {
            ui.heading("Diagnostics");
            ui.horizontal_wrapped(|ui| {
                if ui.button("Get properties").clicked()
                    && let Ok(d) = self.device()
                {
                    self.command(Invocation::adb(
                        &d.serial,
                        &["shell", "getprop"],
                        "Read properties",
                    ));
                }
                if ui.button("Battery").clicked()
                    && let Ok(d) = self.device()
                {
                    self.command(Invocation::adb(
                        &d.serial,
                        &["shell", "dumpsys", "battery"],
                        "Read battery",
                    ));
                }
                if ui.button("Logcat snapshot").clicked()
                    && let Ok(d) = self.device()
                {
                    self.command(Invocation::adb(
                        &d.serial,
                        &["logcat", "-d", "-t", "1000"],
                        "Read logcat",
                    ));
                }
                if ui.button("Clear logcat").clicked()
                    && let Ok(d) = self.device()
                {
                    self.prepare(flash::simple(
                        &d,
                        "Clear logcat",
                        &["logcat", "-c"],
                        true,
                        false,
                    ));
                }
                if ui.button("Mirror with scrcpy").clicked()
                    && let Ok(d) = self.device()
                {
                    let mut call = Invocation::global(
                        Tool::External(self.config.scrcpy.clone()),
                        &["-s", &d.serial],
                        "Mirror selected device",
                    );
                    call.timeout = 86400;
                    self.command(call);
                }
                if ui.button("Start Shizuku").clicked()
                    && let Ok(d) = self.device()
                {
                    self.prepare(flash::simple(
                        &d,
                        "Start Shizuku",
                        &[
                            "shell",
                            "sh",
                            "/sdcard/Android/data/moe.shizuku.privileged.api/start.sh",
                        ],
                        true,
                        false,
                    ));
                }
            });
            if self.config.expert {
                ui.horizontal(|ui| {
                    ui.label("ADB shell");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.utility_command).desired_width(420.0),
                    );
                    if ui.button("Review shell command").clicked()
                        && let Ok(d) = self.device()
                    {
                        self.prepare(flash::simple(
                            &d,
                            "Run custom ADB shell command",
                            &["shell", &self.utility_command],
                            true,
                            false,
                        ));
                    }
                });
            }
        });
        ui.add_space(16.0);
        card(ui, |ui| {
            ui.heading("Root configuration editor");
            ui.label(
                RichText::new(
                    "Read or edit a selected /data/adb configuration. Writes require review.",
                )
                .color(muted(ui)),
            );
            let previous_path = self.file_path.clone();
            ui.add_enabled_ui(!self.busy, |ui| {
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("root-config")
                        .selected_text("Common paths")
                        .show_ui(ui, |ui| {
                            for path in [
                                "/data/adb/pif.json",
                                "/data/adb/modules/playintegrityfix/custom.pif.json",
                                "/data/adb/modules/playintegrityfix/custom.pif.prop",
                                "/data/adb/tricky_store/target.txt",
                                "/data/adb/tricky_store/security_patch.txt",
                                "/data/adb/modules/targetedfix/target.txt",
                            ] {
                                if ui.selectable_label(self.file_path == path, path).clicked() {
                                    self.file_path = path.into();
                                }
                            }
                        });
                    ui.add(egui::TextEdit::singleline(&mut self.file_path).desired_width(450.0));
                });
                if previous_path != self.file_path {
                    self.file_content.clear();
                }
                ui.horizontal(|ui| {
                    if ui.button("Read from device").clicked()
                        && let Ok(d) = self.device()
                    {
                        let path = self.file_path.clone();
                        self.run("Reading configuration", move |t, _, _| {
                            let text = services::rooted_file_read(&t, &d, &path)?;
                            Ok(Payload::FileText {
                                serial: d.serial,
                                path,
                                text,
                            })
                        });
                    }
                    if ui
                    .button("Format JSON / JSON5")
                    .on_hover_text(
                        "Converts valid JSON5 to JSON; removes comments and normalizes formatting",
                    )
                    .clicked()
                {
                    match pixelflasher_rust::config::normalize_json(&self.file_content) {
                        Ok(text) => {
                            self.file_content = text;
                            self.status = "Configuration formatted as JSON".into();
                        }
                        Err(e) => self.error = Some(e.to_string()),
                    }
                }
                    if ui.button("Review save to device").clicked() {
                        if (self.file_path.ends_with(".json") || self.file_path.ends_with(".json5"))
                            && pixelflasher_rust::config::normalize_json(&self.file_content)
                                .is_err()
                        {
                            self.error = Some("Configuration JSON is invalid".into());
                        } else {
                            self.prepare(self.device().and_then(|d| {
                                services::rooted_file_write(
                                    &d,
                                    &self.file_path,
                                    &self.file_content,
                                    &self.data,
                                )
                            }));
                        }
                    }
                });
                egui::ScrollArea::vertical()
                    .id_salt("root-configuration-editor")
                    .max_height(160.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.file_content)
                                .code_editor()
                                .desired_width(f32::INFINITY)
                                .desired_rows(8),
                        );
                    });
            });
        });
        ui.add_space(16.0);
        card(ui, |ui| {
            ui.heading("Local inspection tools");
            ui.horizontal(|ui| {
                if ui.button("Inspect boot / AVB image").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("Partition image", &["img"])
                        .pick_file()
                {
                    match image::inspect(&path) {
                        Ok(v) => {
                            self.utility_json = serde_json::to_string_pretty(&v).unwrap_or_default()
                        }
                        Err(e) => self.error = Some(e.to_string()),
                    }
                }
                if ui.button("Decode binary XML").clicked()
                    && let Some(path) = rfd::FileDialog::new().pick_file()
                {
                    match fs::read(path)
                        .map_err(anyhow::Error::from)
                        .and_then(|v| xml::decode(&v))
                    {
                        Ok(v) => self.utility_json = v,
                        Err(e) => self.error = Some(e.to_string()),
                    }
                }
                if ui.button("Build properties to JSON").clicked()
                    && let Some(path) = rfd::FileDialog::new().pick_file()
                {
                    match fs::read_to_string(path) {
                        Ok(text) => {
                            self.utility_json =
                                serde_json::to_string_pretty(&xml::build_print(&text))
                                    .unwrap_or_default()
                        }
                        Err(e) => self.error = Some(e.to_string()),
                    }
                }
            });
            if !self.utility_json.is_empty() {
                egui::ScrollArea::vertical()
                    .id_salt("local-inspection-result")
                    .max_height(240.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.utility_json)
                                .code_editor()
                                .desired_width(f32::INFINITY),
                        );
                    });
                if ui.button("Save inspection result").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .set_file_name("inspection.json")
                        .save_file()
                    && let Err(e) = fs::write(path, &self.utility_json)
                {
                    self.error = Some(e.to_string());
                }
            }
        });
    }
    pub(super) fn settings_page(&mut self, ui: &mut egui::Ui) {
        self.agent_settings(ui);
        self.saved_devices(ui);
        card(ui, |ui| {
            ui.heading("Android platform tools");
            ui.label("Select one folder containing both adb and fastboot.");
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.tools_path).desired_width(600.0));
                if ui.button("Browse folder").clicked()
                    && let Some(p) = rfd::FileDialog::new().pick_folder()
                {
                    self.tools_path = p.display().to_string();
                }
            });
            path_picker(ui, &mut self.scrcpy_path, "scrcpy", &["exe"]);
            ui.horizontal(|ui| {
                if ui.button("Save settings").clicked() {
                    self.config.platform_tools = PathBuf::from(&self.tools_path);
                    self.config.scrcpy = PathBuf::from(&self.scrcpy_path);
                    self.config.root_solution = self.solution.label().into();
                    match self.config.save(&self.data) {
                        Ok(_) => self.status = "Settings saved".into(),
                        Err(e) => self.error = Some(e.to_string()),
                    }
                }
                if ui.button("Check ADB version").clicked() {
                    self.config.platform_tools = PathBuf::from(&self.tools_path);
                    self.command(Invocation::global(Tool::Adb, &["version"], "Verify ADB"));
                }
                if ui.button("Check fastboot version").clicked() {
                    self.config.platform_tools = PathBuf::from(&self.tools_path);
                    self.command(Invocation::global(
                        Tool::Fastboot,
                        &["--version"],
                        "Verify fastboot",
                    ));
                }
            });
            ui.label(
                RichText::new(format!("Workspace data: {}", self.data.display()))
                    .size(11.0)
                    .color(muted(ui)),
            );
        });
        ui.add_space(16.0);
        card(ui, |ui| {
            ui.heading("Personal tools");
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut self.tool_name);
            });
            path_picker(ui, &mut self.tool_path, "Executable", &["exe"]);
            ui.horizontal(|ui| {
                ui.label("Arguments (JSON array)");
                ui.add(
                    egui::TextEdit::singleline(&mut self.tool_args)
                        .hint_text("[\"--help\"]")
                        .desired_width(400.0),
                );
                if ui.button("Add tool").clicked() {
                    match serde_json::from_str::<Vec<String>>(if self.tool_args.is_empty() {
                        "[]"
                    } else {
                        &self.tool_args
                    }) {
                        Ok(args) => {
                            self.config.tools.push(PersonalTool {
                                name: self.tool_name.clone(),
                                executable: PathBuf::from(&self.tool_path),
                                args,
                                directory: self.data.clone(),
                            });
                            let _ = self.config.save(&self.data);
                        }
                        Err(e) => self.error = Some(e.to_string()),
                    }
                }
            });
            let tools = self.config.tools.clone();
            for (i, t) in tools.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(&t.name);
                    if ui.button("Run").clicked() {
                        let mut invocation =
                            Invocation::global(Tool::External(t.executable.clone()), &[], &t.name);
                        invocation.args = t.args.clone();
                        invocation.directory = Some(t.directory.clone());
                        self.command(invocation);
                    }
                    if ui.small_button("Remove").clicked() {
                        self.config.tools.remove(i);
                        let _ = self.config.save(&self.data);
                    }
                });
            }
        });
        ui.add_space(16.0);
        card(ui, |ui| {
            ui.heading("About this port");
            ui.label("PixelFlasher Rust Edition · native Rust / egui desktop application");
            ui.label("Based on badabing2005/PixelFlasher revision 081286d6baf8ac969df29e8b0ab3a5b6ae4cfb54.");
            ui.label("No Python runtime is required. Android platform-tools and selected root-manager assets remain external dependencies.");
            ui.label("See docs/PARITY.md for implemented and remaining workflows.");
            ui.hyperlink_to(
                "Upstream source",
                "https://github.com/badabing2005/PixelFlasher",
            );
        });
    }
}
