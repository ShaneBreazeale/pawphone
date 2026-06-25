//! Contacts screen: the cast of cats, each a tappable row with avatar, signal
//! badge and online/napping status.

use egui::RichText;

use crate::app::PawPhoneApp;
use crate::models::Cat;
use crate::views::theme;

impl PawPhoneApp {
    pub(crate) fn contacts_view(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.label(RichText::new("CONTACTS").color(theme::GREEN).strong());
        ui.add_space(6.0);

        let cats = self.cats.clone();
        for cat in &cats {
            if render_row(ui, cat).clicked() {
                self.open_thread(cat.id);
            }
            ui.add_space(6.0);
        }
    }
}

fn render_row(ui: &mut egui::Ui, cat: &Cat) -> egui::Response {
    let frame = egui::Frame::new()
        .fill(theme::PANEL)
        .inner_margin(egui::Margin::same(10))
        .corner_radius(egui::CornerRadius::same(6));

    let inner = frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let (av, _) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::hover());
            theme::paint_cat(ui.painter(), av, theme::persona_color(cat.persona));
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.label(RichText::new(&cat.name).color(theme::GREEN).strong());
                ui.label(
                    RichText::new(format!(
                        "{}  ·  {}  {:.0}%",
                        cat.status.label(),
                        signal_bars(cat.signal),
                        cat.signal * 100.0
                    ))
                    .small()
                    .color(theme::GREEN_DIM),
                );
            });
        });
    });

    inner.response.interact(egui::Sense::click())
}

/// Five-cell signal badge, e.g. `▮▮▮▯▯`.
fn signal_bars(s: f32) -> String {
    let lit = (s * 5.0).round().clamp(0.0, 5.0) as usize;
    let mut out = String::with_capacity(5);
    for i in 0..5 {
        out.push(if i < lit { '▮' } else { '▯' });
    }
    out
}
