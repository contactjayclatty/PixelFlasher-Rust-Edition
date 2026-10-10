use super::*;
impl PixelFlasher {
    pub(super) fn app_privileges(&mut self, ui: &mut egui::Ui) {
        if self.config.expert && self.solution == Solution::Magisk {
            card(ui, |ui| {
                ui.heading("Magisk app privileges");
                ui.label(RichText::new("Applies only to devices running Magisk. KernelSU permissions are managed by its manager.").color(muted(ui)));
                ui.horizontal_wrapped(|ui| {
                    for (label, add) in [("Add to denylist", true), ("Remove from denylist", false)]
                    {
                        if ui
                            .add_enabled(
                                self.selected_packages.len() == 1,
                                egui::Button::new(label),
                            )
                            .clicked()
                        {
                            let package = self.selected_packages.iter().next().cloned().unwrap();
                            self.prepare(
                                self.device()
                                    .and_then(|d| advanced::denylist(&d, &package, add)),
                            );
                        }
                    }
                    for (label, policy, minutes) in [
                        ("Grant SU forever", 2, 0),
                        ("Grant SU 10 min", 2, 10),
                        ("Deny SU", 1, 0),
                        ("Revoke SU policy", 0, 0),
                    ] {
                        if ui
                            .add_enabled(
                                self.selected_packages.len() == 1,
                                egui::Button::new(label),
                            )
                            .clicked()
                        {
                            let package = self.selected_packages.iter().next().cloned().unwrap();
                            let uid = self
                                .packages
                                .iter()
                                .find(|p| p.name == package)
                                .map(|p| p.uid.clone())
                                .unwrap_or_default();
                            self.prepare(self.device().and_then(|d| {
                                let mut plan = advanced::magisk_permission(
                                    &d, &uid, policy, minutes, true, true,
                                )?;
                                plan.expected_uids.push((package.clone(), uid.clone()));
                                Ok(plan)
                            }));
                        }
                    }
                });
            });
            ui.add_space(16.0);
        }
    }
    pub(super) fn magisk_backup_page(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.heading("Magisk stock-image backups");
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!self.busy, egui::Button::new("Load backups"))
                    .clicked()
                    && let Ok(d) = self.device()
                {
                    self.run("Reading Magisk backups", move |t, _, _| {
                        Ok(Payload::Backups(advanced::magisk_backups(&t, &d)?))
                    });
                }
                if ui.button("Add selected stock image").clicked() {
                    self.prepare(self.device().and_then(|d| {
                        let image = self.selected_image()?;
                        ensure!(image.patched_with.is_empty(), "Select a stock image");
                        advanced::add_magisk_backup(&d, &image.path, &self.data)
                    }));
                }
            });
            for hash in self.magisk_backups.clone() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&hash).monospace().size(11.0));
                    if ui.small_button("Delete backup").clicked() {
                        self.prepare(
                            self.device()
                                .and_then(|d| advanced::delete_magisk_backup(&d, &hash)),
                        );
                    }
                });
            }
        });
        ui.add_space(16.0);
    }
    pub(super) fn inspection_tools(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.heading("Inspection & support");
            ui.horizontal_wrapped(|ui| {
                if ui.button("Inspect APK manifest").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("Android package", &["apk"])
                        .pick_file()
                {
                    match advanced::apk_manifest(&path) {
                        Ok(v) => self.utility_json = v,
                        Err(e) => self.error = Some(e.to_string()),
                    }
                }
                for (label, online) in [
                    ("Inspect keybox certificates", false),
                    ("Check certificate revocation", true),
                ] {
                    if ui.button(label).clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("Keybox XML", &["xml"])
                            .pick_file()
                    {
                        self.run("Inspecting certificate metadata", move |_, _, _| {
                            let report = advanced::keybox_report(&path, online)?;
                            Ok(Payload::InspectionText(serde_json::to_string_pretty(
                                &report,
                            )?))
                        });
                    }
                }
                if ui.button("Device integrity diagnostics").clicked()
                    && let Ok(d) = self.device()
                {
                    self.run("Reading integrity diagnostics", move |t, _, _| {
                        Ok(Payload::InspectionText(advanced::integrity_report(&t, &d)?))
                    });
                }
                if ui.button("Export support ZIP").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .set_file_name("pixelflasher-support.zip")
                        .save_file()
                {
                    let mut config = self.config.clone();
                    for d in &self.devices {
                        if !config.devices.iter().any(|v| v.serial == d.serial) {
                            config.devices.push(pixelflasher_rust::config::SavedDevice {
                                serial: d.serial.clone(),
                                label: String::new(),
                                enabled: true,
                            });
                        }
                    }
                    if let Err(e) = advanced::support_zip(&path, &self.log, &config) {
                        self.error = Some(e.to_string());
                    }
                }
            });
            ui.label(RichText::new("Private key material is excluded from certificate reports. Device root configuration is kept out of the activity log.").size(11.0).color(muted(ui)));
        });
        ui.add_space(16.0);
    }
    pub(super) fn saved_devices(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.heading("Import existing image records");
            ui.label("Read a PixelFlasher BOOT database without changing it. Only files matching the recorded SHA-1 are imported.");
            if ui.button("Import PixelFlasher database").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("PixelFlasher database", &["db"])
                    .pick_file()
            {
                let data = self.data.clone();
                self.run("Importing legacy library", move |_, _, _| {
                    let (count, skipped) = Library::open(&data)?.import_legacy(&path)?;
                    Ok(Payload::LibraryChanged(format!(
                        "Imported {count} image records. Skipped {} entries.\n{}",
                        skipped.len(),
                        skipped.join("\n")
                    )))
                });
            }
        });
        ui.add_space(16.0);
        card(ui, |ui| {
            ui.heading("Remembered devices");
            ui.label(
                RichText::new("Discovered devices can be labeled and included in normal scans.")
                    .color(muted(ui)),
            );
            let mut changed = false;
            for d in &mut self.config.devices {
                ui.horizontal(|ui| {
                    changed |= ui.checkbox(&mut d.enabled, "Enabled").changed();
                    ui.label(format!(
                        "…{}",
                        d.serial
                            .chars()
                            .rev()
                            .take(4)
                            .collect::<String>()
                            .chars()
                            .rev()
                            .collect::<String>()
                    ));
                    changed |= ui
                        .add(egui::TextEdit::singleline(&mut d.label).hint_text("Device label"))
                        .changed();
                });
            }
            if changed {
                let _ = self.config.save(&self.data);
            }
        });
        ui.add_space(16.0);
    }
}
