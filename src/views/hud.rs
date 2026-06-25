//! The PawLink HUD — a full-screen takeover during `send()`. A 1990s satellite
//! uplink terminal crossed with a cat toy: orbiting birds, filling signal bars,
//! live scrolling telemetry, and a phosphor-green CRT wash.

use std::f32::consts::TAU;

use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, pos2, vec2};

use crate::app::PawPhoneApp;
use crate::phase::PawLinkPhase;
use crate::views::theme;

impl PawPhoneApp {
    pub(crate) fn hud_overlay(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let time = ctx.input(|i| i.time) as f32;

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::INK))
            .show_inside(ui, |ui| {
                let area = ui.available_rect_before_wrap();
                let (resp, painter) = ui.allocate_painter(area.size(), egui::Sense::hover());
                let rect = resp.rect;

                painter.rect_filled(rect, egui::CornerRadius::ZERO, theme::INK);

                // Header.
                painter.text(
                    pos2(rect.left() + 20.0, rect.top() + 16.0),
                    Align2::LEFT_TOP,
                    "PAWLINK UPLINK TERMINAL",
                    FontId::monospace(18.0),
                    theme::GREEN,
                );
                let cat_name = self
                    .pending_cat_id
                    .and_then(|id| self.cat(id))
                    .map(|c| format!("{} {}", c.avatar, c.name))
                    .unwrap_or_default();
                painter.text(
                    pos2(rect.right() - 20.0, rect.top() + 18.0),
                    Align2::RIGHT_TOP,
                    cat_name,
                    FontId::monospace(15.0),
                    theme::GREEN_DIM,
                );

                // Orbit + central cat.
                let center = pos2(rect.center().x, rect.top() + rect.height() * 0.32);
                draw_orbit(&painter, center, &self.live_phase, time);

                // Big phase label.
                painter.text(
                    pos2(center.x, center.y + 118.0),
                    Align2::CENTER_CENTER,
                    self.live_phase.label(),
                    FontId::monospace(26.0),
                    phase_color(&self.live_phase),
                );

                // Signal bars.
                draw_signal_bars(&painter, center.x, center.y + 152.0, self.live_signal);

                // Scrolling telemetry.
                draw_telemetry(&painter, rect, &self.live_telemetry, time);

                // CRT scanlines on top of everything.
                theme::scanlines(&painter, rect);

                // Footer disclaimer.
                painter.text(
                    pos2(rect.center().x, rect.bottom() - 12.0),
                    Align2::CENTER_BOTTOM,
                    "PawLink is a fictional constellation. No actual cats were connected to space.",
                    FontId::monospace(10.0),
                    theme::GREEN_FAINT,
                );
            });

        // Interactive failure card (real widgets, not painter) over the HUD.
        // Suppressed while a resend is in flight (`busy`) so its RESEND/DISMISS
        // buttons can't be clicked again during the worker wake-up window.
        let failed = match &self.live_phase {
            PawLinkPhase::Failed(e) if !self.busy => Some(*e),
            _ => None,
        };
        if let Some(err) = failed {
            let mut resend = false;
            let mut dismiss = false;
            egui::Area::new(egui::Id::new("pawlink_fail_card"))
                .anchor(Align2::CENTER_CENTER, vec2(0.0, 70.0))
                .show(&ctx, |ui| {
                    egui::Frame::new()
                        .fill(theme::PANEL)
                        .stroke(Stroke::new(1.0, theme::RED))
                        .inner_margin(egui::Margin::same(16))
                        .corner_radius(egui::CornerRadius::same(8))
                        .show(ui, |ui| {
                            ui.set_max_width(360.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("✕ {}", err.headline()))
                                        .color(theme::RED)
                                        .strong(),
                                );
                                ui.label(
                                    egui::RichText::new(err.detail())
                                        .small()
                                        .color(theme::GREEN_DIM),
                                );
                                ui.add_space(10.0);
                                ui.horizontal(|ui| {
                                    if ui.button("⟳ RESEND").clicked() {
                                        resend = true;
                                    }
                                    if ui.button("✕ DISMISS").clicked() {
                                        dismiss = true;
                                    }
                                });
                            });
                        });
                });
            if resend {
                self.resend();
            }
            if dismiss {
                self.dismiss_hud();
            }
        }
    }
}

fn phase_color(phase: &PawLinkPhase) -> Color32 {
    match phase {
        PawLinkPhase::Failed(_) => theme::RED,
        PawLinkPhase::Connected => theme::GREEN,
        PawLinkPhase::Locked | PawLinkPhase::Receiving => theme::CYAN,
        _ => theme::AMBER,
    }
}

fn draw_orbit(painter: &Painter, center: Pos2, phase: &PawLinkPhase, time: f32) {
    let (found, total) = match phase {
        PawLinkPhase::Acquiring { found, total } => (*found, *total),
        PawLinkPhase::Idle | PawLinkPhase::PoweringUp => (0, 12),
        _ => (12, 12),
    };

    // Two orbit ellipses.
    let (rx, ry) = (155.0_f32, 72.0_f32);
    draw_ellipse(painter, center, rx, ry, theme::GREEN_FAINT);
    draw_ellipse(painter, center, rx * 0.66, ry * 0.66, theme::GREEN_FAINT);

    // Central body (the cat, naturally).
    painter.circle_filled(center, 26.0, theme::PANEL_HI);
    painter.circle_stroke(center, 26.0, Stroke::new(1.0, theme::GREEN_DIM));
    painter.text(center, Align2::CENTER_CENTER, "🐱", FontId::proportional(30.0), theme::GREEN);

    // Orbiting birds.
    for i in 0..total {
        let scale = if i % 2 == 0 { 1.0 } else { 0.66 };
        let speed = 0.5 + (i % 3) as f32 * 0.18;
        let ang = time * speed + i as f32 * TAU / total.max(1) as f32;
        let p = pos2(center.x + ang.cos() * rx * scale, center.y + ang.sin() * ry * scale);
        let lit = i < found;
        if lit {
            painter.line_segment(
                [p, center],
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(86, 255, 150, 36)),
            );
            painter.circle_filled(p, 4.5, theme::GREEN);
        } else {
            painter.circle_filled(p, 3.0, theme::GREEN_FAINT);
        }
    }
}

fn draw_ellipse(painter: &Painter, center: Pos2, rx: f32, ry: f32, color: Color32) {
    const N: usize = 72;
    let mut pts: Vec<Pos2> = Vec::with_capacity(N);
    for i in 0..N {
        let a = i as f32 * TAU / N as f32;
        pts.push(pos2(center.x + a.cos() * rx, center.y + a.sin() * ry));
    }
    painter.add(egui::Shape::closed_line(pts, Stroke::new(1.0, color)));
}

fn draw_signal_bars(painter: &Painter, cx: f32, baseline: f32, signal: f32) {
    const BARS: usize = 12;
    let lit = (signal * BARS as f32).round().clamp(0.0, BARS as f32) as usize;
    let (bw, gap) = (10.0_f32, 4.0_f32);
    let total_w = BARS as f32 * (bw + gap) - gap;
    let x0 = cx - total_w / 2.0;
    for i in 0..BARS {
        let h = 8.0 + i as f32 * 3.0;
        let x = x0 + i as f32 * (bw + gap);
        let r = Rect::from_min_size(pos2(x, baseline - h), vec2(bw, h));
        let color = if i < lit { theme::GREEN } else { theme::GREEN_FAINT };
        painter.rect_filled(r, egui::CornerRadius::same(1), color);
    }
}

fn draw_telemetry(painter: &Painter, rect: Rect, lines: &[String], time: f32) {
    const VISIBLE: usize = 10;
    let base_y = rect.bottom() - 54.0;
    // newest line at the bottom, dimming upward
    for (idx, line) in lines.iter().rev().take(VISIBLE).enumerate() {
        let y = base_y - idx as f32 * 16.0;
        let alpha = (255i32 - idx as i32 * 22).clamp(40, 255) as u8;
        let color = Color32::from_rgba_unmultiplied(86, 255, 150, alpha);
        painter.text(
            pos2(rect.left() + 20.0, y),
            Align2::LEFT_BOTTOM,
            format!("> {line}"),
            FontId::monospace(13.0),
            color,
        );
    }
    // Blinking prompt caret.
    let caret = if (time * 2.0) as i64 % 2 == 0 { "> ▮" } else { ">" };
    painter.text(
        pos2(rect.left() + 20.0, base_y + 18.0),
        Align2::LEFT_BOTTOM,
        caret,
        FontId::monospace(13.0),
        theme::GREEN,
    );
}
