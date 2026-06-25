//! Settings: realism profile, mute, volume, haptics, and a tongue-in-cheek
//! constellation status readout.

use egui::RichText;

use crate::app::PawPhoneApp;
use crate::phase::RealismProfile;
use crate::views::theme;

impl PawPhoneApp {
    pub(crate) fn settings_view(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.label(RichText::new("SETTINGS").color(theme::GREEN).strong());
        ui.separator();

        ui.label(RichText::new("REALISM PROFILE").color(theme::GREEN_DIM));
        for p in RealismProfile::ALL {
            let selected = self.profile == p;
            if ui
                .selectable_label(selected, format!("{} — {}", p.name(), p.blurb()))
                .clicked()
            {
                self.profile = p;
            }
        }

        ui.separator();
        ui.label(RichText::new("AUDIO").color(theme::GREEN_DIM));

        let mut muted = self.audio.is_muted();
        if ui.checkbox(&mut muted, "mute all audio").changed() {
            self.audio.set_muted(muted);
        }

        let mut vol = self.audio.volume();
        if ui.add(egui::Slider::new(&mut vol, 0.0..=1.0).text("volume")).changed() {
            self.audio.set_volume(vol);
        }
        if !self.audio.active {
            ui.label(
                RichText::new("⚠ no audio device — running silent")
                    .small()
                    .color(theme::AMBER),
            );
        }

        ui.separator();
        ui.label(RichText::new("HAPTICS").color(theme::GREEN_DIM));
        let mut hap = self.haptics_on();
        if ui.checkbox(&mut hap, "haptics enabled").changed() {
            self.set_haptics(hap);
        }
        ui.label(
            RichText::new("(desktop Macs have no haptic engine — this gates a graceful no-op)")
                .small()
                .color(theme::GREEN_FAINT),
        );

        ui.separator();
        ui.label(RichText::new("CONSTELLATION STATUS").color(theme::GREEN).strong());
        ui.label(RichText::new("12/12 birds operational · 3 napping").color(theme::GREEN_DIM));
        ui.label(
            RichText::new("uplink treats: 87% · purr-rate nominal · 1 bird chasing a moth")
                .small()
                .color(theme::GREEN_FAINT),
        );
    }
}
