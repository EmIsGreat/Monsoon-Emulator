use crossbeam_channel::Sender;
use egui::Context;

use crate::frontend::egui::config::AppConfig;
use crate::frontend::egui::tiles::{Pane, add_pane_if_missing};
use crate::frontend::messages::AsyncFrontendMessage;
use crate::frontend::util::{spawn_rom_picker, spawn_savestate_picker};

pub fn add_menu_bar(
    ctx: &Context,
    config: &AppConfig,
    async_sender: &Sender<AsyncFrontendMessage>,
    tree: &mut egui_tiles::Tree<Pane>,
) {
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Load Rom").clicked() {
                    spawn_rom_picker(async_sender, config.user_config.previous_rom_dir.as_ref());
                }

                ui.menu_button("Savestates", |ui| {
                    if config.user_config.loaded_rom.is_some() && ui.button("Save State").clicked()
                    {
                        let _ = async_sender.send(AsyncFrontendMessage::CreateSavestate);
                    }

                    if ui.button("Load State").clicked() {
                        // Use the new multistep savestate loading flow
                        spawn_savestate_picker(
                            async_sender,
                            config.user_config.previous_savestate_dir.as_ref(),
                        );
                    }

                    if config.user_config.loaded_rom.is_some()
                        && ui.button("Browse Saves...").clicked()
                    {
                        ui.separator();
                        let _ = async_sender.send(AsyncFrontendMessage::OpenSaveBrowser);
                        ui.close();
                    }
                });
            });
            ui.menu_button("Console", |ui| {
                if ui.button("Reset").clicked() {
                    let _ = async_sender.send(AsyncFrontendMessage::Reset);
                }
                if ui.button("Power cycle").clicked() {
                    let _ = async_sender.send(AsyncFrontendMessage::PowerOff);
                    let _ = async_sender.send(AsyncFrontendMessage::LoadRom(
                        config.user_config.loaded_rom.clone().map(|r| r.1),
                    ));
                    let _ = async_sender.send(AsyncFrontendMessage::PowerOn);
                }
                if !config.console_config.is_powered {
                    if ui.button("Power On").clicked() {
                        let _ = async_sender.send(AsyncFrontendMessage::PowerOn);
                    }
                } else if ui.button("Power Off").clicked() {
                    let _ = async_sender.send(AsyncFrontendMessage::PowerOff);
                }
            });
            ui.menu_button("View", |ui| {
                if ui.button("Options").clicked() {
                    add_pane_if_missing(tree, Pane::Options);
                    ui.close();
                }
                if ui.button("Keybindings").clicked() {
                    add_pane_if_missing(tree, Pane::Keybindings);
                    ui.close();
                }
                ui.separator();
                ui.label("Debug Viewers");
                if ui.button("Palettes").clicked() {
                    add_pane_if_missing(tree, Pane::Palettes);
                    ui.close();
                }
                if ui.button("Pattern Tables").clicked() {
                    add_pane_if_missing(tree, Pane::PatternTables);
                    ui.close();
                }
                if ui.button("Nametables").clicked() {
                    add_pane_if_missing(tree, Pane::Nametables);
                    ui.close();
                }
                if ui.button("Sprite Viewer").clicked() {
                    add_pane_if_missing(tree, Pane::Sprites);
                    ui.close();
                }
            });
        });
    });
}
