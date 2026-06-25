//! Visual identity: a green-phosphor CRT terminal that happens to be a cat
//! messenger. Monospace everywhere, dark background, scanline-friendly.

use egui::{Color32, FontFamily, FontId, TextStyle};

pub const BG: Color32 = Color32::from_rgb(6, 12, 9);
pub const PANEL: Color32 = Color32::from_rgb(10, 19, 14);
pub const PANEL_HI: Color32 = Color32::from_rgb(16, 30, 22);
pub const GREEN: Color32 = Color32::from_rgb(86, 255, 150);
pub const GREEN_DIM: Color32 = Color32::from_rgb(66, 176, 112);
pub const GREEN_FAINT: Color32 = Color32::from_rgb(30, 74, 52);
pub const AMBER: Color32 = Color32::from_rgb(255, 201, 92);
pub const RED: Color32 = Color32::from_rgb(255, 96, 90);
pub const CYAN: Color32 = Color32::from_rgb(120, 230, 230);
pub const INK: Color32 = Color32::from_rgb(8, 14, 11);

/// Apply the terminal theme + monospace text styles to the whole app.
pub fn install(ctx: &egui::Context) {
    ctx.global_style_mut(|style| {
        style.text_styles = [
            (TextStyle::Heading, FontId::new(22.0, FontFamily::Monospace)),
            (TextStyle::Body, FontId::new(15.0, FontFamily::Monospace)),
            (TextStyle::Monospace, FontId::new(14.0, FontFamily::Monospace)),
            (TextStyle::Button, FontId::new(15.0, FontFamily::Monospace)),
            (TextStyle::Small, FontId::new(11.0, FontFamily::Monospace)),
        ]
        .into();

        let v = &mut style.visuals;
        v.dark_mode = true;
        v.panel_fill = BG;
        v.window_fill = PANEL;
        v.extreme_bg_color = INK;
        v.faint_bg_color = PANEL_HI;
        v.override_text_color = Some(GREEN_DIM);
        v.hyperlink_color = CYAN;
        v.selection.bg_fill = GREEN_FAINT;
        v.selection.stroke = egui::Stroke::new(1.0, GREEN);

        // Make widgets read like terminal chrome.
        v.widgets.inactive.bg_fill = PANEL_HI;
        v.widgets.inactive.weak_bg_fill = PANEL_HI;
        v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, GREEN_DIM);
        v.widgets.hovered.bg_fill = GREEN_FAINT;
        v.widgets.hovered.weak_bg_fill = GREEN_FAINT;
        v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, GREEN);
        v.widgets.active.bg_fill = GREEN_FAINT;
        v.widgets.active.weak_bg_fill = GREEN_FAINT;
        v.widgets.active.fg_stroke = egui::Stroke::new(1.0, GREEN);
    });
}

/// Paint faint horizontal CRT scanlines over a rect. Cheap and atmospheric.
pub fn scanlines(painter: &egui::Painter, rect: egui::Rect) {
    let line = Color32::from_rgba_unmultiplied(0, 0, 0, 60);
    let mut y = rect.top();
    while y < rect.bottom() {
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            egui::Stroke::new(1.0, line),
        );
        y += 3.0;
    }
}
