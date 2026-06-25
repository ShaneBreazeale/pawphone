//! Visual identity: a green-phosphor CRT terminal that happens to be a cat
//! messenger. Monospace everywhere, dark background, scanline-friendly.

use egui::{Color32, FontFamily, FontId, TextStyle};

use crate::models::CatPersona;

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

/// A per-persona tint, so each cat's drawn avatar reads as a distinct character.
pub fn persona_color(p: CatPersona) -> Color32 {
    match p {
        CatPersona::Aloof => GREEN_DIM,
        CatPersona::Needy => CYAN,
        CatPersona::Chaotic => AMBER,
        CatPersona::FoodObsessed => GREEN,
    }
}

/// Draw a little vector cat-face avatar inside `rect`, tinted `color`.
///
/// We draw it rather than rely on emoji: egui ships only a small *monochrome*
/// emoji subset, so most emoji (🟠, 🧤, 🐱, 🐾 …) render as tofu boxes. A painted
/// face always renders, scales cleanly, and suits the terminal look.
pub fn paint_cat(painter: &egui::Painter, rect: egui::Rect, color: Color32) {
    let c = rect.center();
    let s = rect.width().min(rect.height());
    let hr = s * 0.30; // head radius
    let stroke = egui::Stroke::new((s * 0.05).max(1.0), color);
    let ear_fill = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 40);

    // Ears (triangles), faintly filled + outlined.
    let ear_h = hr;
    let l = vec![
        egui::pos2(c.x - hr * 0.78, c.y - hr * 0.30),
        egui::pos2(c.x - hr * 0.55, c.y - hr * 0.30 - ear_h),
        egui::pos2(c.x - hr * 0.05, c.y - hr * 0.55),
    ];
    let r = vec![
        egui::pos2(c.x + hr * 0.78, c.y - hr * 0.30),
        egui::pos2(c.x + hr * 0.55, c.y - hr * 0.30 - ear_h),
        egui::pos2(c.x + hr * 0.05, c.y - hr * 0.55),
    ];
    painter.add(egui::Shape::convex_polygon(l, ear_fill, stroke));
    painter.add(egui::Shape::convex_polygon(r, ear_fill, stroke));

    // Head.
    painter.circle_stroke(c, hr, stroke);

    // Eyes + nose.
    let eye_dx = hr * 0.42;
    let eye_y = c.y - hr * 0.05;
    let dot = (s * 0.04).max(1.0);
    painter.circle_filled(egui::pos2(c.x - eye_dx, eye_y), dot, color);
    painter.circle_filled(egui::pos2(c.x + eye_dx, eye_y), dot, color);
    painter.circle_filled(egui::pos2(c.x, c.y + hr * 0.25), dot * 0.9, color);

    // Whiskers.
    let wy = c.y + hr * 0.25;
    let wl = hr * 1.15;
    painter.line_segment([egui::pos2(c.x - hr * 0.25, wy), egui::pos2(c.x - wl, wy - hr * 0.18)], stroke);
    painter.line_segment([egui::pos2(c.x - hr * 0.25, wy), egui::pos2(c.x - wl, wy + hr * 0.22)], stroke);
    painter.line_segment([egui::pos2(c.x + hr * 0.25, wy), egui::pos2(c.x + wl, wy - hr * 0.18)], stroke);
    painter.line_segment([egui::pos2(c.x + hr * 0.25, wy), egui::pos2(c.x + wl, wy + hr * 0.22)], stroke);
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
