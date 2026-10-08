#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::Local;
use crossbeam_channel::{Receiver, unbounded};
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use std::io::BufRead;
use std::thread;

#[derive(Clone)]
struct Interpretation {
    text: String,
    color: egui::Color32,
}

#[derive(Clone)]
struct LogEntry {
    timestamp: String,
    raw_text: String,
    human_readable: Option<Interpretation>,
}

struct TelemetryViewerApp {
    log_rx: Receiver<LogEntry>,
    logs: Vec<LogEntry>,
    auto_scroll: bool,
}

impl TelemetryViewerApp {
    fn new(cc: &eframe::CreationContext<'_>, log_rx: Receiver<LogEntry>) -> Self {
        // Set up dark theme
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        Self {
            log_rx,
            logs: Vec::new(),
            auto_scroll: true,
        }
    }
}

impl eframe::App for TelemetryViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Drain new logs
        let mut new_logs = false;
        while let Ok(entry) = self.log_rx.try_recv() {
            self.logs.push(entry);
            new_logs = true;
        }

        // Prevent unbounded memory growth
        if self.logs.len() > 10_000 {
            let overflow = self.logs.len() - 10_000;
            self.logs.drain(0..overflow);
        }

        if new_logs {
            ctx.request_repaint(); // keep updating the UI if logs arrive
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Expressive Control Telemetry Viewer");
                ui.separator();
                ui.checkbox(&mut self.auto_scroll, "Auto-scroll");
                if ui.button("Clear").clicked() {
                    self.logs.clear();
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let row_height = ui.text_style_height(&egui::TextStyle::Body);

            let mut table = TableBuilder::new(ui)
                .striped(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::initial(100.0).at_least(80.0))
                .column(Column::initial(400.0).resizable(true))
                .column(Column::remainder().at_least(200.0))
                .min_scrolled_height(0.0)
                .auto_shrink(false);

            if self.auto_scroll && !self.logs.is_empty() {
                table = table.scroll_to_row(self.logs.len() - 1, Some(egui::Align::BOTTOM));
            }

            table
                .header(20.0, |mut header| {
                    header.col(|ui| {
                        ui.strong("Time");
                    });
                    header.col(|ui| {
                        ui.strong("Raw / Rust Struct");
                    });
                    header.col(|ui| {
                        ui.strong("Interpretation");
                    });
                })
                .body(|body| {
                    body.rows(row_height, self.logs.len(), |mut row| {
                        let row_idx = row.index();
                        let entry = &self.logs[row_idx];

                        row.col(|ui| {
                            ui.label(egui::RichText::new(&entry.timestamp).weak());
                        });

                        row.col(|ui| {
                            let is_input = entry.raw_text.contains("Input:");
                            let prefix = if is_input { "" } else { "🎹 " };
                            let formatted_raw = format!("{}{}", prefix, entry.raw_text);

                            let mut raw_text = egui::RichText::new(&formatted_raw);
                            // Dim sensor injection logs
                            if is_input {
                                raw_text = raw_text.color(egui::Color32::from_gray(140));
                            }

                            let raw_lbl = ui.label(raw_text);
                            raw_lbl.on_hover_text(&entry.raw_text);
                        });

                        row.col(|ui| {
                            if let Some(interp) = &entry.human_readable {
                                ui.label(
                                    egui::RichText::new(&interp.text)
                                        .color(interp.color)
                                        .strong(),
                                );
                            } else {
                                ui.label("");
                            }
                        });
                    });
                });
        });
    }
}

/// A very naive parser just to show the concept. It looks for known keywords in the string.
fn interpret_log(text: &str) -> Option<Interpretation> {
    if text.contains("Input: Touch") {
        Some(Interpretation {
            text: "👆 Capacitive Touch".into(),
            color: egui::Color32::LIGHT_BLUE,
        })
    } else if text.contains("Input: Released") {
        Some(Interpretation {
            text: "👇 Touch Released".into(),
            color: egui::Color32::GRAY,
        })
    } else if text.contains("NoteOn") {
        Some(Interpretation {
            text: "🎵 Note Pressed".into(),
            color: egui::Color32::GREEN,
        })
    } else if text.contains("NoteOff") {
        Some(Interpretation {
            text: "🛑 Note Released".into(),
            color: egui::Color32::LIGHT_RED,
        })
    } else if text.contains("PitchBend") {
        Some(Interpretation {
            text: "〰️ Pitch Bend Event".into(),
            color: egui::Color32::KHAKI,
        })
    } else if text.contains("Timbre") {
        Some(Interpretation {
            text: "🎛️ Timbre/Brightness (CC 74)".into(),
            color: egui::Color32::GOLD,
        })
    } else if text.contains("ChannelPressure") {
        Some(Interpretation {
            text: "🎈 Channel Pressure".into(),
            color: egui::Color32::ORANGE,
        })
    } else if text.contains("Position3D") {
        Some(Interpretation {
            text: "📏 ToF Sensor Read".into(),
            color: egui::Color32::LIGHT_BLUE,
        })
    } else {
        None
    }
}

fn main() -> Result<(), eframe::Error> {
    // 1. Setup channel
    let (tx, rx) = unbounded();

    // 2. Spawn a thread to read from stdin
    // This allows `cargo run | cargo run -p telemetry-viewer` to work seamlessly!
    thread::spawn(move || {
        let stdin = std::io::stdin();
        for text in stdin.lock().lines().map_while(Result::ok) {
            // Filter out cargo build junk if desired, or keep it
            let human = interpret_log(&text);
            let entry = LogEntry {
                timestamp: Local::now().format("%H:%M:%S.%3f").to_string(),
                raw_text: text,
                human_readable: human,
            };
            let _ = tx.send(entry);
        }
    });

    // 3. Start UI
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 600.0])
            .with_title("Telemetry Viewer"),
        ..Default::default()
    };

    eframe::run_native(
        "Telemetry Viewer",
        options,
        Box::new(|cc| Ok(Box::new(TelemetryViewerApp::new(cc, rx)))),
    )
}
