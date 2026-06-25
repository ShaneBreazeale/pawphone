//! Thread screen: chat bubbles (yours on the right) plus a compose bar with a
//! meow phrasebook picker and free-text that gets "translated" on send.

use egui::{Align, Layout, RichText};

use crate::app::PawPhoneApp;
use crate::models::{DeliveryStatus, Message, Screen};
use crate::personality;
use crate::views::theme;

impl PawPhoneApp {
    pub(crate) fn thread_view(&mut self, ui: &mut egui::Ui) {
        let cat_id = match self.screen {
            Screen::Thread { cat_id } => cat_id,
            _ => return,
        };
        let cat = self.cat(cat_id);

        ui.horizontal(|ui| {
            if ui.button("‹ back").clicked() {
                self.screen = Screen::Contacts;
            }
            if let Some(c) = &cat {
                ui.label(RichText::new(format!("{} {}", c.avatar, c.name)).color(theme::GREEN).strong());
                ui.label(RichText::new(c.status.label()).small().color(theme::GREEN_DIM));
            }
        });
        ui.separator();

        // Compose bar pinned to the bottom; messages fill the rest.
        egui::Panel::bottom("compose_bar")
            .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(egui::Margin::same(8)))
            .show_inside(ui, |ui| self.compose_bar(ui));

        egui::CentralPanel::default().show_inside(ui, |ui| {
            let msgs = self.current_messages.clone();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if msgs.is_empty() {
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new("no transmissions yet · say something meow-ful")
                                    .small()
                                    .color(theme::GREEN_FAINT),
                            );
                        });
                    }
                    for m in &msgs {
                        render_bubble(ui, m);
                    }
                });
        });
    }

    fn compose_bar(&mut self, ui: &mut egui::Ui) {
        let mut to_send = None;
        let mut send_typed = false;

        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("phrasebook")
                .selected_text("phrasebook ▾")
                .show_ui(ui, |ui| {
                    for p in personality::phrasebook() {
                        if ui
                            .button(format!("{}   {}", p.meow, p.subtitle))
                            .clicked()
                        {
                            to_send = Some(p);
                        }
                    }
                });

            let send_clicked = ui
                .with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let send = ui.button("SEND");
                    let te = ui.add(
                        egui::TextEdit::singleline(&mut self.compose)
                            .hint_text("type — get meows")
                            .desired_width(ui.available_width()),
                    );
                    let entered = te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    send.clicked() || entered
                })
                .inner;
            if send_clicked {
                send_typed = true;
            }
        });

        if let Some(p) = to_send {
            self.send_phrase(p);
        }
        if send_typed {
            self.send_typed();
        }
    }
}

fn render_bubble(ui: &mut egui::Ui, m: &Message) {
    let layout = if m.from_me {
        Layout::right_to_left(Align::Min)
    } else {
        Layout::left_to_right(Align::Min)
    };
    ui.with_layout(layout, |ui| {
        let fill = if m.from_me { theme::GREEN_FAINT } else { theme::PANEL_HI };
        egui::Frame::new()
            .fill(fill)
            .inner_margin(egui::Margin::same(8))
            .corner_radius(egui::CornerRadius::same(8))
            .show(ui, |ui| {
                ui.set_max_width(330.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(&m.meow).color(theme::GREEN).strong());
                    ui.label(RichText::new(&m.subtitle).small().italics().color(theme::GREEN_DIM));
                    if m.from_me {
                        ui.label(RichText::new(m.status.glyph()).small().color(status_color(m.status)));
                    }
                });
            });
    });
    ui.add_space(4.0);
}

fn status_color(s: DeliveryStatus) -> egui::Color32 {
    match s {
        DeliveryStatus::Sending => theme::AMBER,
        DeliveryStatus::Delivered => theme::GREEN_DIM,
        DeliveryStatus::Lost(_) => theme::RED,
    }
}
