use std::collections::HashSet;

use crossbeam_channel::Sender;
use egui::WidgetText;
use egui_tiles::{Behavior, SimplificationOptions, TileId, Tiles, UiResponse};
use monsoon_core::emulation::ppu_util::EmulatorFetchable;
use serde::{Deserialize, Serialize};

use crate::channel_emu::{ChannelEmulator, FETCH_DEPS};
use crate::frontend::egui::config::AppConfig;
use crate::frontend::egui::textures::EmuTextures;
use crate::frontend::egui::ui::{
    render_emulator_output, render_keybindings, render_nametable, render_options, render_palettes,
    render_pattern_table, render_rom_header, render_soam_viewer, render_sprite_viewer,
};
use crate::frontend::messages::AsyncFrontendMessage;

/// The different pane types that can be displayed in the tile tree
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pane {
    /// Main emulator output - cannot be closed
    EmulatorOutput,
    /// Options panel - closeable, can be reopened via menu
    Options,
    /// Pattern tables viewer (both tables side by side) - closeable debug
    /// viewer
    PatternTables,
    /// Nametables viewer (all 4 nametables in a grid) - closeable debug viewer
    Nametables,
    Palettes,
    Sprites,
    SoamSprites,
    RomHeader,
    /// Keybindings configuration - closeable, can be reopened via menu
    Keybindings,
}

impl Pane {
    /// Returns whether this pane can be closed
    pub fn is_closable(&self) -> bool {
        match self {
            Pane::EmulatorOutput => false,
            Pane::Options
            | Pane::PatternTables
            | Pane::Nametables
            | Pane::Palettes
            | Pane::Keybindings
            | Pane::Sprites
            | Pane::SoamSprites
            | Pane::RomHeader => true,
        }
    }

    /// Returns the tab title for this pane
    pub fn title(&self) -> &'static str {
        match self {
            Pane::EmulatorOutput => "Emulator Output",
            Pane::Options => "Options",
            Pane::PatternTables => "Pattern Tables",
            Pane::Nametables => "Nametables",
            Pane::Palettes => "Palettes",
            Pane::Keybindings => "Keybindings",
            Pane::Sprites => "Sprites",
            Pane::SoamSprites => "SOAM Sprites",
            Pane::RomHeader => "ROM Header",
        }
    }
}

/// Behavior implementation for the tile tree.
pub struct TreeBehavior<'a> {
    pub config: &'a mut AppConfig,
    pub emu_textures: &'a EmuTextures,
    pub async_sender: &'a Sender<AsyncFrontendMessage>,
    pub keybindings_changed: bool,
}

impl<'a> TreeBehavior<'a> {
    pub fn new(
        config: &'a mut AppConfig,
        emu_textures: &'a EmuTextures,
        async_sender: &'a Sender<AsyncFrontendMessage>,
    ) -> Self {
        Self {
            config,
            emu_textures,
            async_sender,
            keybindings_changed: false,
        }
    }
}

impl Behavior<Pane> for TreeBehavior<'_> {
    fn pane_ui(&mut self, ui: &mut egui::Ui, _: TileId, pane: &mut Pane) -> UiResponse {
        match pane {
            Pane::EmulatorOutput => {
                render_emulator_output(ui, self.emu_textures, self.config.is_effectively_paused());
            }
            Pane::Options => {
                render_options(ui, self.config);
            }
            Pane::PatternTables => {
                render_pattern_table(ui, self.config, self.emu_textures, self.async_sender);
            }
            Pane::Nametables => {
                render_nametable(ui, self.emu_textures);
            }
            Pane::Palettes => {
                render_palettes(ui, self.config, self.emu_textures, self.async_sender)
            }
            Pane::Keybindings => {
                self.keybindings_changed |= render_keybindings(ui, self.config);
            }
            Pane::Sprites => render_sprite_viewer(ui, self.emu_textures),
            Pane::SoamSprites => render_soam_viewer(ui, self.config, self.emu_textures),
            Pane::RomHeader => render_rom_header(ui, self.config),
        }
        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &Pane) -> WidgetText { pane.title().into() }

    fn is_tab_closable(&self, tiles: &Tiles<Pane>, tile_id: TileId) -> bool {
        if let Some(egui_tiles::Tile::Pane(pane)) = tiles.get(tile_id) {
            pane.is_closable()
        } else {
            true
        }
    }

    fn on_tab_close(&mut self, _tiles: &mut Tiles<Pane>, _tile_id: TileId) -> bool {
        // Always allow close for closable panes
        true
    }

    fn simplification_options(&self) -> SimplificationOptions {
        SimplificationOptions {
            all_panes_must_have_tabs: true,
            ..Default::default()
        }
    }
}

/// Create the initial tile tree with the default layout
pub fn create_tree() -> egui_tiles::Tree<Pane> {
    let mut tiles = Tiles::default();

    // Create the main emulator output pane
    let emulator_output = tiles.insert_pane(Pane::EmulatorOutput);

    // Create the options pane
    let options = tiles.insert_pane(Pane::Options);

    // Create a horizontal layout with emulator on left, options on right
    let root = tiles.insert_horizontal_tile(vec![emulator_output, options]);

    // Set shares so emulator takes most of the space
    if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Linear(linear))) =
        tiles.get_mut(root)
    {
        linear.shares.set_share(emulator_output, 3.0);
        linear.shares.set_share(options, 1.0);
    }

    egui_tiles::Tree::new("emulator_tiles", root, tiles)
}

/// Find a pane in the tree by its type
pub fn find_pane(tiles: &Tiles<Pane>, pane_type: &Pane) -> Option<TileId> {
    for (tile_id, tile) in tiles.iter() {
        if let egui_tiles::Tile::Pane(pane) = tile
            && pane == pane_type
        {
            return Some(*tile_id);
        }
    }
    None
}

/// Add a new pane to the tree if it doesn't exist
pub fn add_pane_if_missing(tree: &mut egui_tiles::Tree<Pane>, pane_type: Pane) {
    if find_pane(&tree.tiles, &pane_type).is_none() {
        let new_tile_id = tree.tiles.insert_pane(pane_type);

        // Add to root container
        if let Some(root) = tree.root()
            && let Some(egui_tiles::Tile::Container(container)) = tree.tiles.get_mut(root)
        {
            match container {
                egui_tiles::Container::Tabs(tabs) => {
                    tabs.add_child(new_tile_id);
                    tabs.set_active(new_tile_id);
                }
                egui_tiles::Container::Linear(linear) => {
                    linear.children.push(new_tile_id);
                }
                egui_tiles::Container::Grid(grid) => {
                    grid.add_child(new_tile_id);
                }
            }
        }
    }
}

/// Compute required debug fetches based on which panes are visible
pub fn compute_required_fetches_from_tree(
    tree: &egui_tiles::Tree<Pane>,
    config: &AppConfig,
) -> HashSet<EmulatorFetchable> {
    let mut explicit_fetches = HashSet::new();

    // Check if pattern tables pane is visible
    if find_pane(&tree.tiles, &Pane::PatternTables).is_some() {
        explicit_fetches.insert(EmulatorFetchable::Tiles(None));
    }

    // Check if nametables pane is visible
    if find_pane(&tree.tiles, &Pane::Nametables).is_some() {
        explicit_fetches.insert(EmulatorFetchable::Nametables(None));
    }

    if find_pane(&tree.tiles, &Pane::Palettes).is_some() {
        explicit_fetches.insert(EmulatorFetchable::Palettes(None));
    }

    if find_pane(&tree.tiles, &Pane::Sprites).is_some() {
        explicit_fetches.insert(EmulatorFetchable::Sprites(None));
    }

    if find_pane(&tree.tiles, &Pane::SoamSprites).is_some() && config.is_effectively_paused() {
        explicit_fetches.insert(EmulatorFetchable::SoamSprites(None));
    }

    if !explicit_fetches.is_empty() {
        if let Some(fetch_deps) = FETCH_DEPS.get() {
            ChannelEmulator::compute_required_fetches(&explicit_fetches, fetch_deps)
        } else {
            // FETCH_DEPS not yet initialized, return explicit fetches as-is
            explicit_fetches
        }
    } else {
        explicit_fetches
    }
}
