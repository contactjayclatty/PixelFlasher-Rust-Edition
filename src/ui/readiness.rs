use super::*;
impl PixelFlasher {
    pub(super) fn readiness(&mut self, ui: &mut egui::Ui) {
        let inspected = self
            .device()
            .is_ok_and(|d| !d.properties.is_empty() || !d.variables.is_empty());
        let inputs = self.firmware.is_some() || self.image.is_some();
        card(ui, |ui| {
            ui.label(
                RichText::new("YOUR UPDATE WORKFLOW")
                    .size(10.0)
                    .color(muted(ui)),
            );
            ui.add_space(6.0);
            ui.columns(4, |columns| {
                for (index, title, detail, ready, page) in [
                    (
                        0,
                        "01  Platform tools",
                        if self.config.tools_ready() {
                            "ADB + fastboot found"
                        } else {
                            "Choose tools folder"
                        },
                        self.config.tools_ready(),
                        Page::Settings,
                    ),
                    (
                        1,
                        "02  Inspect device",
                        if inspected {
                            "Device inspected"
                        } else {
                            "Scan, select and inspect"
                        },
                        inspected,
                        Page::Overview,
                    ),
                    (
                        2,
                        "03  Prepare images",
                        if inputs {
                            "Input selected"
                        } else {
                            "Import or drop a file"
                        },
                        inputs,
                        Page::Firmware,
                    ),
                    (
                        3,
                        "04  Review operation",
                        if self.pending.is_some() {
                            "Plan awaiting confirmation"
                        } else {
                            "Review before execution"
                        },
                        self.pending.is_some(),
                        Page::Flash,
                    ),
                ] {
                    let col = &mut columns[index];
                    col.label(RichText::new(title).strong().color(if ready {
                        accent(col)
                    } else {
                        col.visuals().text_color()
                    }));
                    col.label(RichText::new(detail).size(11.0).color(muted(col)));
                    if col
                        .small_button(if ready { "Open" } else { "Continue" })
                        .clicked()
                    {
                        self.page = page;
                    }
                }
            });
        });
    }
}
