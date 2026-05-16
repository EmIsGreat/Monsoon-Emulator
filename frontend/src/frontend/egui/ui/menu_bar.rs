use crossbeam_channel::Sender;
use egui::Ui;

use crate::frontend::egui::config::AppConfig;
use crate::frontend::egui::keybindings::OnKeyAction;
use crate::frontend::egui::ui::widgets::HotKeyButton;
use crate::frontend::messages::AsyncFrontendMessage;

pub fn add_menu_bar(
    ui: &mut Ui,
    config: &mut AppConfig,
    async_sender: &Sender<AsyncFrontendMessage>,
) {
    egui::Panel::top("menu_bar").show_inside(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                ui.add(HotKeyButton::for_action(
                    OnKeyAction::LoadRom,
                    config,
                    async_sender,
                ));

                ui.menu_button("Savestates", |ui| {
                    ui.add(HotKeyButton::for_action(
                        OnKeyAction::LoadSavestate,
                        config,
                        async_sender,
                    ));

                    if config.console_config.loaded_rom.is_some() {
                        ui.add(HotKeyButton::for_action(
                            OnKeyAction::CreateSavestate,
                            config,
                            async_sender,
                        ));

                        ui.separator();
                        ui.add(HotKeyButton::for_action(
                            OnKeyAction::BrowseSavestates,
                            config,
                            async_sender,
                        ));
                    }
                });
            });
            ui.menu_button("Edit", |ui| {
                ui.add(HotKeyButton::for_action(
                    OnKeyAction::OpenOptionsMenu,
                    config,
                    async_sender,
                ));
                ui.add(HotKeyButton::for_action(
                    OnKeyAction::OpenKeybindingsMenu,
                    config,
                    async_sender,
                ));
            });
            ui.menu_button("Console", |ui| {
                ui.add(HotKeyButton::for_action(
                    OnKeyAction::Reset,
                    config,
                    async_sender,
                ));
                ui.add(HotKeyButton::for_action(
                    OnKeyAction::PowerToggle,
                    config,
                    async_sender,
                ));
                ui.add(HotKeyButton::for_action(
                    OnKeyAction::PowerCycle,
                    config,
                    async_sender,
                ));
            });
            ui.menu_button("View", |ui| {
                ui.label("Debug Viewers");

                ui.add(HotKeyButton::for_action(
                    OnKeyAction::OpenPaletteViewer,
                    config,
                    async_sender,
                ));
                ui.add(HotKeyButton::for_action(
                    OnKeyAction::OpenPatternTableViewer,
                    config,
                    async_sender,
                ));
                ui.add(HotKeyButton::for_action(
                    OnKeyAction::OpenNametableViewer,
                    config,
                    async_sender,
                ));
                ui.add(HotKeyButton::for_action(
                    OnKeyAction::OpenSpriteViewer,
                    config,
                    async_sender,
                ));
                ui.add(HotKeyButton::for_action(
                    OnKeyAction::OpenSoamViewer,
                    config,
                    async_sender,
                ));
                ui.add(HotKeyButton::for_action(
                    OnKeyAction::OpenRomHeaderViewer,
                    config,
                    async_sender,
                ));
                ui.add(HotKeyButton::for_action(
                    OnKeyAction::OpenRegistersViewer,
                    config,
                    async_sender,
                ));
            });
        });
    });
}
