use super::*;
impl PixelFlasher {
    pub(super) fn root_studio_header(&mut self, ui: &mut egui::Ui) -> bool {
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Provider engine");
                if ui
                    .add_enabled(
                        !self.busy && self.selected.is_some(),
                        egui::Button::new("Detect installed managers"),
                    )
                    .clicked()
                    && let Ok(d) = self.device()
                {
                    self.run("Inspecting root provider evidence", move |t, _, _| {
                        Ok(Payload::RootContext(providers::inspect(&t, &d)?))
                    });
                }
            });
            let old = self.solution;
            egui::ComboBox::from_id_salt("studio-provider")
                .selected_text(self.solution.label())
                .show_ui(ui, |ui| {
                    for id in Solution::ALL {
                        ui.selectable_value(&mut self.solution, id, id.label());
                    }
                });
            if old != self.solution {
                self.patch_method = providers::PatchMethod::Classic;
                self.root_version = providers::provider(self.solution).stable_versions()[0].into();
            }
            let adapter = providers::provider(self.solution);
            egui::ComboBox::from_id_salt("provider-version")
                .selected_text(&self.root_version)
                .show_ui(ui, |ui| {
                    for version in adapter.stable_versions() {
                        ui.selectable_value(&mut self.root_version, (*version).into(), *version);
                    }
                });
            ui.horizontal(|ui| {
                ui.label("Patch method");
                ui.selectable_value(
                    &mut self.patch_method,
                    providers::PatchMethod::Classic,
                    "Classic",
                );
                if matches!(self.solution, Solution::KernelSu | Solution::KernelSuNext) {
                    ui.selectable_value(
                        &mut self.patch_method,
                        providers::PatchMethod::KernelSuV2,
                        "V2: boot only",
                    );
                }
            });
            let caps = adapter.capabilities(&self.root_version);
            ui.label(
                RichText::new(format!(
                    "{:?} adapter coverage · Hardware unverified · {}",
                    caps.confidence,
                    caps.targets.join(" / ")
                ))
                .color(warning_color(ui)),
            );
            if let Some(context) = &self.root_context {
                ui.label(format!(
                    "Evidence: {:?} · daemon: {}",
                    context.confidence,
                    if context.daemon.is_empty() {
                        "unknown"
                    } else {
                        &context.daemon
                    }
                ));
                for m in &context.managers {
                    ui.label(format!(
                        "Installed manager: {} {}",
                        m.provider.label(),
                        m.version
                    ));
                }
                if context.confidence == providers::Confidence::Conflicting {
                    ui.colored_label(warning_color(ui),"Multiple managers installed. Select the intended provider explicitly; installed APKs alone do not prove active root.");
                }
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !self.busy && self.selected.is_some(),
                        egui::Button::new("Open official manager"),
                    )
                    .clicked()
                {
                    self.prepare(self.device().and_then(|d| adapter.manager_plan(&d)));
                }
                ui.hyperlink_to(
                    "Official releases",
                    format!("https://github.com/{}/releases", self.solution.repo()),
                );
            });
        });
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            for (i, label) in ["Install & patch", "Modules", "Permissions", "Recovery"]
                .iter()
                .enumerate()
            {
                ui.selectable_value(&mut self.root_tab, i, *label);
            }
        });
        ui.add_space(12.0);
        if self.root_tab == 2 {
            self.app_privileges(ui);
            card(ui, |ui| {
                ui.heading("Provider-aware permissions");
                ui.label("Select packages in Applications for Magisk policy changes. Other providers use the official manager until native profile controls are verified.");
            });
            return false;
        }
        if self.root_tab == 3 {
            if self.solution == Solution::Magisk {
                self.magisk_backup_page(ui);
            }
            card(ui, |ui| {
                ui.heading("Recovery workflow");
                ui.label("Restore a matching stock image from Firmware & Images, review its device and slot, then inspect final state. No automatic rollback is claimed.");
                if ui.button("Open Backups & Recovery").clicked() {
                    self.page = Page::Backups;
                }
            });
            return false;
        }
        true
    }
    pub(super) fn verified_backup_panel(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.heading("Verified backup sets");
            ui.label("List partitions below, then select the partitions to include. Each image is checked against its device-side SHA-256.");
            let partitions = self.partitions.clone();
            egui::ScrollArea::vertical()
                .id_salt("backup-set-table")
                .max_height(200.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        sized_text(ui, 28.0, RichText::new("").strong());
                        sized_text(ui, 220.0, RichText::new("Partition").strong());
                    });
                    if partitions.is_empty() {
                        ui.label("List partitions to build a backup set.");
                    }
                    for partition in &partitions {
                        ui.horizontal(|ui| {
                            let mut selected = self.backup_partitions.contains(partition);
                            if ui
                                .add_sized(
                                    [28.0, 18.0],
                                    egui::Checkbox::without_text(&mut selected),
                                )
                                .changed()
                            {
                                if selected {
                                    self.backup_partitions.insert(partition.clone());
                                } else {
                                    self.backup_partitions.remove(partition);
                                }
                            }
                            sized_text(ui, 220.0, partition.as_str());
                        });
                    }
                });
            if ui
                .add_enabled(
                    !self.busy && !self.backup_partitions.is_empty(),
                    egui::Button::new("Review backup set"),
                )
                .clicked()
                && let Some(directory) = rfd::FileDialog::new().pick_folder()
            {
                let device = self.device();
                let parts = self.backup_partitions.iter().cloned().collect::<Vec<_>>();
                self.prepare_async("Preparing backup set", move || {
                    let job = pixelflasher_rust::recovery::prepare(&device?, &parts, &directory)?;
                    Ok(Pending {
                        plan: OperationPlan::freeze(job.plan)?,
                        patch: None,
                        backup: Some(job.output),
                    })
                });
            }
            if ui
                .add_enabled(
                    !self.busy && self.selected.is_some(),
                    egui::Button::new("Verify and review matching restore"),
                )
                .clicked()
                && let Some(directory) = rfd::FileDialog::new().pick_folder()
            {
                let device = self.device();
                self.prepare_async("Verifying backup set", move || {
                    Ok(Pending {
                        plan: OperationPlan::freeze(pixelflasher_rust::recovery::restore_plan(
                            &directory, &device?,
                        )?)?,
                        patch: None,
                        backup: None,
                    })
                });
            }
        });
        ui.add_space(12.0);
    }
    pub(super) fn review_queue(&mut self, ui: &mut egui::Ui) {
        let Ok(library) = Library::open(&self.data) else {
            return;
        };
        let Ok(requests) = library.review_requests() else {
            return;
        };
        if requests.is_empty() {
            return;
        }
        card(ui, |ui| {
            ui.heading("Agent review queue");
            ui.label("Select and freshly inspect the requested device before preparing a plan. Execution still requires typed local confirmation.");
            for (id, intent) in requests {
                ui.push_id(&id, |ui| {
                    ui.label(format!("{intent:?}"));
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(!self.busy, egui::Button::new("Prepare local review"))
                            .clicked()
                        {
                            let result = (|| -> Result<Plan> {
                                let d = self.device()?;
                                let requested = match &intent {
                                    operation::OperationIntent::OpenManager {
                                        device_id, ..
                                    }
                                    | operation::OperationIntent::FlashImage {
                                        device_id, ..
                                    }
                                    | operation::OperationIntent::PatchRoot { device_id, .. } => {
                                        device_id
                                    }
                                };
                                ensure!(
                                    pixelflasher_rust::agent::device_id(&d.serial) == *requested,
                                    "Select the device associated with this request"
                                );
                                ensure!(!d.product.is_empty(), "Inspect this device first");
                                match &intent {
                                    operation::OperationIntent::OpenManager {
                                        provider, ..
                                    } => providers::provider(*provider).manager_plan(&d),
                                    operation::OperationIntent::FlashImage {
                                        artifact_id,
                                        partition,
                                        ..
                                    } => {
                                        let image = self
                                            .images
                                            .iter()
                                            .find(|a| {
                                                format!("image-{}", a.info.sha256) == *artifact_id
                                            })
                                            .ok_or_else(|| {
                                                anyhow::anyhow!("Artifact no longer exists")
                                            })?;
                                        ensure!(
                                            image.product.is_empty() || image.product == d.product,
                                            "Artifact belongs to a different device product"
                                        );
                                        flash::boot_image(
                                            &d,
                                            &image.path,
                                            partition,
                                            false,
                                            &FlashOptions::default(),
                                        )
                                    }
                                    _ => {
                                        anyhow::bail!("Select patch inputs locally in Root Studio")
                                    }
                                }
                            })();
                            if result.is_ok() {
                                let _ = library.dismiss_review(&id);
                            }
                            self.prepare(result);
                        }
                        if ui.button("Dismiss").clicked() {
                            let _ = library.dismiss_review(&id);
                        }
                    });
                });
            }
        });
        ui.add_space(12.0);
    }
    pub(super) fn journal_panel(&mut self, ui: &mut egui::Ui) {
        self.review_queue(ui);
        if let Ok(rows) = Library::open(&self.data).and_then(|l| l.recent_operations()) {
            card(ui, |ui| {
                ui.heading("Recent operations");
                if rows.is_empty() {
                    ui.label(RichText::new("Approved operations will appear here. Interrupted operations are never replayed automatically.").color(muted(ui)));
                }
                for row in rows.iter().take(5) {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&row.title).strong());
                        ui.label(RichText::new(&row.status).color(accent(ui)));
                    });
                    ui.label(RichText::new(&row.verification).size(11.0).color(muted(ui)));
                }
            });
        }
    }
    pub(super) fn agent_settings(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.heading("Local agent integration");
            if ui
                .checkbox(
                    &mut self.config.mcp_enabled,
                    "Enable local stdio MCP inspection and planning",
                )
                .changed()
            {
                let _ = self.config.save(&self.data);
            }
            ui.label("No AI model or cloud account is required. Agents cannot approve or execute device changes, run arbitrary commands, or access secret buffers.");
            ui.label("Configure your client to run pixelflasher-mcp.exe. Review requests appear in this desktop application's queue.");
        });
        ui.add_space(12.0);
        card(ui, |ui| {
            ui.heading("Appearance");
            let mut changed = ui
                .checkbox(&mut self.config.compact, "Compact density")
                .changed();
            changed |= ui
                .checkbox(&mut self.config.dark, "Dark graphite theme")
                .changed();
            if changed {
                theme(ui.ctx(), self.config.dark);
                let _ = self.config.save(&self.data);
            }
        });
        ui.add_space(12.0);
    }
}
