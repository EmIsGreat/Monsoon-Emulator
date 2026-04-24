/// Main application entry point for the egui frontend.
///
/// This module provides the main `EguiApp` struct and the `run` function
/// to start the frontend. The implementation is split across several
/// submodules for maintainability:
///
/// - `egui::config`: Configuration types
/// - `egui::fps_counter`: FPS tracking
/// - `egui::input`: Input handling
/// - `egui::message_handlers`: Async and emulator message processing
/// - `egui::textures`: Texture management
/// - `egui::tiles`: Tile tree behavior and pane management
/// - `egui::ui`: UI rendering components
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt::{Debug, Formatter};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;


use crossbeam_channel::{Receiver, Sender};
use eframe::Frame;
use egui::{Context, Id, Style, Ui, ViewportCommand, Visuals};
use monsoon_core::declare_renderers;
use monsoon_core::emulation::nes::Nes;
use monsoon_core::emulation::ppu_util::{EmulatorFetchable, PaletteData, TileData, TILE_COUNT};
use monsoon_core::emulation::rom::ExpansionDevice;
use monsoon_core::emulation::savestate::SaveState;
use monsoon_core::emulation::screen_renderer::{
    NoneRenderer, RendererRegistration, ScreenRenderer,
};
use monsoon_core::util::ToBytes;
use monsoon_default_renderers::LookupPaletteRenderer;
use web_time::Instant;

use crate::channel_emu::ChannelEmulator;
use crate::frontend::egui::config::{AppConfig, AppSpeed};
use crate::frontend::egui::fps_counter::FpsCounter;
use crate::frontend::egui::input::handle_keyboard_input;
use crate::frontend::egui::message_handlers::async_handler::extract_timestamp;
use crate::frontend::egui::message_handlers::{AsyncMessageHandler, EmulatorMessageHandler};
use crate::frontend::egui::textures::EmuTextures;
use crate::frontend::egui::tiles::{
    compute_required_fetches_from_tree, create_tree, find_pane, Pane, TreeBehavior,
};
use crate::frontend::egui::ui::{
    add_menu_bar, add_status_bar, render_save_browser, render_savestate_dialogs,
};
use crate::frontend::messages::{
    AsyncFrontendMessage, FrontendEvent, LoadedRom, SavestateLoadContext,
};
use crate::frontend::persistence::{get_egui_storage_path, load_config, PersistentConfig};
use crate::frontend::storage::{Storage, StorageKey};
use crate::frontend::{storage, util};
use crate::messages::{EmulatorMessage, FrontendMessage, SaveType};

/// Key used for storing egui_tiles tree state in egui's persistence
const EGUI_TILES_TREE_KEY: &str = "emulator_tiles_tree";

/// Interval between periodic autosaves (5 minutes)
const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// Maximum number of autosaves to keep per game
const MAX_AUTOSAVES_PER_GAME: usize = 1024;

declare_renderers!(LookupPaletteRenderer, NoneRenderer);

/// Shared deque for frontend events that can be pushed from UI components
pub type FrontendEventQueue = Rc<RefCell<VecDeque<FrontendEvent>>>;

/// Main egui application state.
///
/// Uses `RendererKind` for runtime-switchable rendering. The renderer can be
/// changed at runtime by updating `config.view_config.renderer`.
pub struct EguiApp {
    pub(crate) channel_emu: ChannelEmulator,
    pub(crate) to_emulator: Sender<FrontendMessage>,
    pub(crate) from_emulator: Receiver<EmulatorMessage>,
    pub(crate) from_async: Receiver<AsyncFrontendMessage>,
    pub(crate) async_sender: Sender<AsyncFrontendMessage>,
    /// Queue for visual/frontend-only events processed on each update
    pub(crate) event_queue: FrontendEventQueue,
    pub(crate) emu_textures: EmuTextures,
    pub(crate) fps_counter: FpsCounter,
    accumulator: Duration,
    pub(crate) config: AppConfig,
    /// The tile tree for docking behavior
    pub(crate) tree: egui_tiles::Tree<Pane>,
    /// Track if pattern tables was visible last frame to detect when it becomes
    /// visible
    pattern_tables_was_visible: bool,
    /// Track if nametables was visible last frame to detect when it becomes
    /// visible
    nametables_was_visible: bool,
    /// Time of last periodic autosave
    last_autosave: Instant,
    /// Track if window was focused last frame to detect focus loss
    was_focused: bool,
    was_effectively_paused: bool,
}

impl EguiApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        loaded_config: Option<PersistentConfig>,
        channel_emu: ChannelEmulator,
        to_emulator: Sender<FrontendMessage>,
        from_emulator: Receiver<EmulatorMessage>,
        to_async: Sender<AsyncFrontendMessage>,
        from_async: Receiver<AsyncFrontendMessage>,
    ) -> Self {
        // Try to restore the tile tree from egui's storage, fall back to default
        let tree = cc
            .storage
            .and_then(|storage| {
                eframe::get_value::<egui_tiles::Tree<Pane>>(storage, EGUI_TILES_TREE_KEY)
            })
            .unwrap_or_else(create_tree);

        // Create default config and apply loaded settings
        let mut config = AppConfig::default();
        if let Some(ref persistent_config) = loaded_config {
            config = persistent_config.into();
        }

        Self {
            channel_emu,
            to_emulator,
            from_emulator,
            from_async,
            async_sender: to_async,
            event_queue: Rc::new(RefCell::new(VecDeque::new())),
            emu_textures: Default::default(),
            fps_counter: Default::default(),
            accumulator: Default::default(),
            config,
            tree,
            pattern_tables_was_visible: false,
            nametables_was_visible: false,
            last_autosave: Instant::now(),
            was_focused: true,
            was_effectively_paused: false,
        }
    }

    /// Calculate the frame budget based on current speed settings
    fn get_frame_budget(&self) -> Duration {
        let speed = self
            .config
            .speed_config
            .app_speed
            .get_fps(&self.config.speed_config);

        if speed == 0.0 {
            return Duration::from_secs(5);
        }

        Duration::from_nanos((1_000_000_000.0 / speed as f64) as u64)
    }

    /// Calculate the debug viewers frame budget based on current speed settings
    fn get_debug_viewers_frame_budget(&self) -> Duration {
        let fps = self
            .config
            .speed_config
            .debug_speed
            .get_fps(&self.config.speed_config);

        if fps == 0.0 {
            return Duration::from_secs(5);
        }

        Duration::from_nanos(1_000_000_000 / fps as u64)
    }

    /// Process messages received from various sources
    fn process_messages(&mut self, ctx: &Context) {
        self.process_frontend_events(ctx);
        // Delegate to trait implementations in message_handlers module
        self.handle_async_messages(ctx);
        self.handle_emulator_messages(ctx);
    }

    /// Process visual/frontend-only events from the deque
    fn process_frontend_events(&mut self, ctx: &Context) {
        let mut queue = self.event_queue.borrow_mut();
        while let Some(event) = queue.pop_front() {
            match event {
                FrontendEvent::ChangeWindowTitle(title) => {
                    ctx.send_viewport_cmd(ViewportCommand::Title(title))
                }
                FrontendEvent::RefreshPalette => {
                    // Only update tile textures if a tile viewer is visible
                    if self.is_tile_viewer_visible() {
                        self.emu_textures.update_tile_textures(
                            ctx,
                            &self.config.view_config.palette_rgb_data,
                            None,
                            None,
                        );
                    }
                }
            }
        }
    }

    pub(crate) fn load_rom(&mut self, data: LoadedRom) {
        let name = data.name.clone();

        let _ = self
            .to_emulator
            .send(FrontendMessage::CreateSaveState(SaveType::Autosave));

        let _ = self
            .to_emulator
            .send(FrontendMessage::LoadRom((data, name.clone())));

        // Extract stem for window title
        let stem = name.rsplit_once('.').map(|(s, _)| s).unwrap_or(&name);
        let window_title = if stem.is_empty() {
            "Monsoon".to_string()
        } else {
            format!("Monsoon - {}", stem)
        };

        self.config.user_config.previous_rom_name = Some(name);

        self.event_queue
            .borrow_mut()
            .push_back(FrontendEvent::ChangeWindowTitle(window_title));
    }

    /// Load a savestate after ROM has been verified/selected
    pub(crate) fn load_savestate_with_rom(
        &mut self,
        context: &SavestateLoadContext,
        rom: LoadedRom,
    ) {
        if self.config.console_config.loaded_rom.is_some() {
            let _ = self
                .to_emulator
                .send(FrontendMessage::CreateSaveState(SaveType::Autosave));
        }

        // First power off, load ROM, power on
        let _ = self.to_emulator.send(FrontendMessage::Power(false));
        self.load_rom(rom);
        let _ = self.to_emulator.send(FrontendMessage::Power(true));

        // Then load the savestate
        let _ = self
            .to_emulator
            .send(FrontendMessage::LoadSaveState(Box::new(
                context.savestate.clone(),
            )));

        // Update config names and directories
        self.config.user_config.previous_savestate_name = Some(context.savestate_name.clone());
        if let Some(ref dir) = context.savestate_dir {
            self.config.user_config.previous_savestate_load_dir = Some(StorageKey::from(dir));
        }
    }

    pub(crate) fn create_auto_save(&self, savestate: Box<SaveState>) {
        if let Some(rom) = &self.config.console_config.loaded_rom {
            let rom_hash = &rom.0.data_checksum;
            let prev_name = &self.config.user_config.previous_rom_name;
            if let Some(prev_name) = prev_name {
                let display_name = util::rom_display_name(prev_name, rom_hash);
                let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
                let key = storage::autosave_key(&display_name, &timestamp);

                // Write savestate using storage
                let data = savestate.to_bytes(None);
                util::spawn_async(async move {
                    let storage = storage::get_storage();
                    let _ = storage.set(&key, data).await;
                });

                // Clean up old autosaves asynchronously to avoid blocking the UI
                Self::cleanup_old_autosaves_async(display_name);
            }
        }
    }

    /// Remove the oldest autosaves if there are more than
    /// MAX_AUTOSAVES_PER_GAME. Runs asynchronously in a background thread
    /// (native) or spawn_local (WASM).
    fn cleanup_old_autosaves_async(display_name: String) {
        util::spawn_async(async move {
            let prefix = storage::autosaves_prefix(&display_name);
            let storage = storage::get_storage();

            if let Ok(entries) = storage.list(&prefix).await {
                let mut autosaves: Vec<_> = entries
                    .into_iter()
                    .filter(|e| e.key.sub_path.ends_with(".sav"))
                    .map(|e| e.key)
                    .collect();

                if autosaves.len() >= MAX_AUTOSAVES_PER_GAME {
                    autosaves.sort_by_key(|a| a.sub_path.clone());

                    let to_delete = autosaves.len() - MAX_AUTOSAVES_PER_GAME + 1;
                    for key in autosaves.into_iter().take(to_delete) {
                        let _ = storage.delete(&key).await;
                    }
                }
            }
        });
    }

    /// Find the newest quicksave key from a list of storage entries.
    /// Shared logic between native sync and WASM async paths.
    pub(crate) fn find_newest_quicksave(
        entries: Vec<storage::StorageMetadata>,
    ) -> Option<StorageKey> {
        let mut quicksave_key: Option<(StorageKey, chrono::NaiveDateTime, u8)> = None;

        for entry in entries {
            if !entry.key.sub_path.ends_with(".sav") {
                continue;
            }

            // Extract filename from the key
            let filename = entry.key.sub_path.rsplit('/').next()?;
            let stem = filename.strip_suffix(".sav")?;

            let time_version = stem.split_once('_')?.1;

            let (timestamp, version) = extract_timestamp(time_version)?;

            let time = chrono::NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d_%H-%M-%S");

            let version = version.parse::<u8>().unwrap_or(0);

            if let Ok(time) = time {
                let should_update = match &quicksave_key {
                    None => true,
                    Some(current) => current.1 < time || (current.1 == time && current.2 < version),
                };

                if should_update {
                    quicksave_key = Some((entry.key, time, version));
                }
            }
        }

        quicksave_key.map(|p| p.0)
    }

    /// Check if the pattern tables pane is visible
    fn is_pattern_tables_visible(&self) -> bool {
        find_pane(&self.tree.tiles, &Pane::PatternTables).is_some()
    }

    /// Check if the nametables pane is visible
    fn is_nametables_visible(&self) -> bool {
        find_pane(&self.tree.tiles, &Pane::Nametables).is_some()
    }

    fn is_sprite_viewer_visible(&self) -> bool {
        find_pane(&self.tree.tiles, &Pane::Sprites).is_some()
    }

    fn is_soam_viewer_visible(&self) -> bool {
        find_pane(&self.tree.tiles, &Pane::SoamSprites).is_some()
            && self.config.is_effectively_paused()
    }

    /// Check if any viewer that needs tile textures is visible
    pub(crate) fn is_tile_viewer_visible(&self) -> bool {
        self.is_pattern_tables_visible()
            || self.is_nametables_visible()
            || self.is_sprite_viewer_visible()
            || self.is_soam_viewer_visible()
    }

    /// Check if pattern tables or nametables viewer just became visible and
    /// force rebuild if so
    fn check_and_handle_viewer_visibility(&mut self, ctx: &Context) {
        let pattern_tables_visible = self.is_pattern_tables_visible();
        let nametables_visible = self.is_nametables_visible();

        // If either viewer just became visible, force full rebuild
        let pattern_just_opened = pattern_tables_visible && !self.pattern_tables_was_visible;
        let nametables_just_opened = nametables_visible && !self.nametables_was_visible;

        if pattern_just_opened || nametables_just_opened {
            self.emu_textures
                .force_rebuild_all_tiles(ctx, &self.config.view_config.palette_rgb_data);
        }

        self.pattern_tables_was_visible = pattern_tables_visible;
        self.nametables_was_visible = nametables_visible;
    }

    /// Detect which palettes changed between old and new palette data
    pub(crate) fn detect_changed_palettes(
        &self,
        new_palette_data: &Option<Box<PaletteData>>,
    ) -> Vec<usize> {
        match (&self.emu_textures.palette_data, new_palette_data) {
            (Some(old), Some(new)) => old
                .colors
                .iter()
                .zip(new.colors.iter())
                .enumerate()
                .filter(|(_, (old_pal, new_pal))| old_pal != new_pal)
                .map(|(idx, _)| idx)
                .collect(),
            (None, Some(_)) => (0..8).collect(), // All palettes are new
            _ => vec![],                         // No update needed
        }
    }

    /// Detect which tiles changed between old and new tile data
    pub(crate) fn detect_changed_tiles(
        &self,
        new_tile_data: &Option<Box<[TileData; TILE_COUNT]>>,
    ) -> Vec<usize> {
        match (&self.emu_textures.tile_data, new_tile_data) {
            (Some(old), Some(new)) => old
                .iter()
                .zip(new.iter())
                .enumerate()
                .filter(|(_, (old_tile, new_tile))| old_tile != new_tile)
                .map(|(idx, _)| idx)
                .collect(),
            (None, Some(_)) => vec![], // All tiles are new - return empty to trigger full rebuild
            _ => vec![],               // No update needed
        }
    }

    /// Update emulator textures and run emulation frames
    fn update_emu_textures(&mut self, ctx: &Context) {
        let now = Instant::now();
        let is_effectively_paused = self.config.is_effectively_paused();
        if is_effectively_paused && !self.was_effectively_paused {
            self.reset_frame_timing_baseline(now);
        }
        self.was_effectively_paused = is_effectively_paused;

        if !is_effectively_paused {
            let delta = now.duration_since(self.emu_textures.last_frame_request);
            self.emu_textures.last_frame_request = now;

            let frame_budget = self.get_frame_budget();
            let is_uncapped = self.config.speed_config.app_speed == AppSpeed::Uncapped
                || ctx.memory(|mem| {
                    mem.data
                        .get_temp(Id::new(AsyncFrontendMessage::Speedup))
                        .unwrap_or(false)
                });

            if !is_uncapped {
                self.accumulator += delta;
            }

            let max_emulation_time = Duration::from_millis(17);

            let emulation_start = Instant::now();

            // Effectively paused, so we skip
            if frame_budget < Duration::from_secs(5) {
                while self.accumulator >= frame_budget || is_uncapped {
                    if let Err(e) = self.channel_emu.execute_frame() {
                        eprintln!("Emulator error: {}", e);
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                        break;
                    }

                    if !is_uncapped {
                        self.accumulator -= frame_budget;
                    }

                    // For uncapped mode, keep running frames until we hit our time budget
                    // For normal modes, check if we've spent too much time and need to drop frames
                    let elapsed = emulation_start.elapsed();
                    if elapsed > max_emulation_time {
                        if !is_uncapped {
                            // Drop backlog only in non-uncapped mode when falling behind
                            self.accumulator = Duration::ZERO;
                        }
                        break;
                    }
                }
            }
        }

        self.request_debug_views(now);
    }

    /// Request debug data from the emulator based on timing and visibility.
    ///
    /// Active fetches (like nametables) are requested on a regular interval.
    /// Passive fetches (like tiles and palettes) are only requested once
    /// initially, then re-requested when the emulator notifies of changes via
    /// `PatternTableChanged` or `PaletteChanged` messages.
    fn request_debug_views(&mut self, now: Instant) {
        let debug_frame_budget = self.get_debug_viewers_frame_budget();

        // Effectively paused, so we skip
        if debug_frame_budget < Duration::from_secs(5)
            && now.duration_since(self.emu_textures.last_debug_request) >= debug_frame_budget
        {
            for to_fetch in &self.config.view_config.required_debug_fetches {
                // Passive fetches (tiles, palettes) are only requested once initially.
                // After that, they're re-requested when the emulator notifies of changes.
                let should_skip_passive = to_fetch.is_passive()
                    && match to_fetch {
                        EmulatorFetchable::Tiles(_) => self.emu_textures.tile_textures.is_some(),
                        EmulatorFetchable::Palettes(_) => self.emu_textures.palette_data.is_some(),
                        _ => false,
                    };

                if !should_skip_passive {
                    let _ = self
                        .to_emulator
                        .send(FrontendMessage::RequestDebugData(to_fetch.clone()));
                }
            }

            self.emu_textures.last_debug_request = Instant::now();
        }
    }

    /// Render any pending dialogs for savestate loading
    fn render_savestate_dialogs_impl(&mut self, ctx: &Context) {
        render_savestate_dialogs(
            ctx,
            &mut self.config.pending_dialogs,
            &self.config.user_config,
            &self.async_sender,
        );
    }

    /// Render the save browser dialog if open
    fn render_save_browser_impl(&mut self, ctx: &Context) {
        render_save_browser(ctx, &mut self.config.pending_dialogs, &self.async_sender);
    }

    /// Check if a periodic autosave should be triggered based on elapsed time
    fn check_periodic_autosave(&mut self) {
        // Only autosave if a ROM is loaded and console is powered
        if self.config.console_config.loaded_rom.is_some()
            && self.config.console_config.is_powered
            && self.last_autosave.elapsed() >= AUTOSAVE_INTERVAL
        {
            // Update timestamp first to prevent overlapping save operations
            self.last_autosave = Instant::now();
            let savestate = self.channel_emu.nes.save_state();
            self.create_auto_save(Box::new(savestate.unwrap()));
        }
    }

    /// Check if window focus was lost and trigger autosave if needed
    fn check_focus_autosave(&mut self, ctx: &Context) {
        let is_focused = ctx.input(|i| i.focused);

        // Only autosave when focus is lost (transition from focused to unfocused)
        // This also resets the periodic timer since we just saved
        if self.was_focused
            && !is_focused
            && self.config.console_config.loaded_rom.is_some()
            && self.config.console_config.is_powered
        {
            // Update timestamp first to prevent overlapping save operations
            self.last_autosave = Instant::now();
            let savestate = self.channel_emu.nes.save_state();
            self.create_auto_save(Box::new(savestate.unwrap()));
        }

        self.was_focused = is_focused;
    }

    pub fn pause(&mut self) {
        self.config.speed_config.is_paused = !self.config.speed_config.is_paused;
        self.reset_frame_timing_baseline(Instant::now());
    }

    /// Reset emulation frame timing baseline to avoid catch-up bursts.
    pub(crate) fn reset_frame_timing_baseline(&mut self, now: Instant) {
        self.emu_textures.last_frame_request = now;
        self.accumulator = Duration::ZERO;
    }

    #[cfg(target_arch = "wasm32")]
    fn persist_config_async(&self) {
        let persistent_config: PersistentConfig = (&self.config).into();
        util::spawn_async(async move {
            if let Err(e) = crate::frontend::persistence::save_config(&persistent_config).await {
                eprintln!("Failed to save configuration: {}", e);
            }
        });
    }
}

impl eframe::App for EguiApp {
    /// Run non-visual per-frame application work.
    ///
    /// This keeps simulation, message processing, and timing updates in
    /// `logic`, while `ui` stays focused on widget/layout rendering and UI
    /// interactions.
    fn logic(&mut self, ctx: &Context, _: &mut Frame) {
        ctx.memory_mut(|mem| {
            mem.data
                .insert_temp::<bool>(Id::new(AsyncFrontendMessage::Speedup), false);
        });

        // Handle keyboard input
        handle_keyboard_input(ctx, &self.async_sender, &mut self.config);
        self.config.sync_dialog_pause_reason();

        // Process pending frontend, async, and emulator messages
        self.process_messages(ctx);

        // Check if tile viewers just became visible and force rebuild if needed
        self.check_and_handle_viewer_visibility(ctx);

        // Update required debug fetches based on visible panes
        self.config.view_config.required_debug_fetches =
            compute_required_fetches_from_tree(&self.tree, &self.config);

        if let Err(e) = self.channel_emu.process_messages() {
            eprintln!("Emulator error: {}", e);
            ctx.send_viewport_cmd(ViewportCommand::Close);
            return;
        }

        self.update_emu_textures(ctx);

        // Check autosave triggers
        self.check_periodic_autosave();
        self.check_focus_autosave(ctx);

        // Request continuous repaint for animation/emulation
        ctx.request_repaint();
    }

    fn ui(&mut self, ui: &mut Ui, _: &mut Frame) {
        add_menu_bar(ui, &mut self.config, &self.async_sender);

        // Status bar at bottom
        add_status_bar(ui, &self.fps_counter, &self.config, &self.emu_textures);

        // Central panel with tile tree
        #[cfg(target_arch = "wasm32")]
        let mut keybindings_changed = false;
        egui::CentralPanel::default().show_inside(ui, |ui| {
            let mut behavior =
                TreeBehavior::new(&mut self.config, &self.emu_textures, &self.async_sender);
            self.tree.ui(&mut behavior, ui);
            #[cfg(target_arch = "wasm32")]
            {
                keybindings_changed = behavior.keybindings_changed;
            }
        });

        // Render any pending savestate dialogs
        self.render_savestate_dialogs_impl(ui);

        // Render save browser dialog if open
        self.render_save_browser_impl(ui);

        #[cfg(target_arch = "wasm32")]
        if keybindings_changed {
            self.persist_config_async();
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // Save the tile tree layout to egui's built-in storage
        eframe::set_value(storage, EGUI_TILES_TREE_KEY, &self.tree);
    }

    fn on_exit(&mut self) {
        // Save configuration before exiting
        #[cfg(not(target_arch = "wasm32"))]
        let persistent_config: PersistentConfig = (&self.config).into();
        // On native, block on the async save to ensure it completes before exit.
        // On WASM, call the async helper since we can't block the browser thread.
        #[cfg(not(target_arch = "wasm32"))]
        {
            let rt = tokio::runtime::Handle::current();
            tokio::task::block_in_place(|| {
                if let Err(e) = rt.block_on(crate::frontend::persistence::save_config(
                    &persistent_config,
                )) {
                    eprintln!("Failed to save configuration: {}", e);
                }
            });
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.persist_config_async();
        }

        let savestate = self.channel_emu.nes.save_state();

        if let Some(state) = savestate {
            self.create_auto_save(Box::new(state));
        }

        let _ = self.to_emulator.send(FrontendMessage::Quit);
    }
}

impl Debug for EguiApp {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { f.write_str("EguiApp") }
}

struct SetupResponse {
    emu: ChannelEmulator,
    to_emu: Sender<FrontendMessage>,
    from_emu: Receiver<EmulatorMessage>,
    from_async: Receiver<AsyncFrontendMessage>,
    async_sender: Sender<AsyncFrontendMessage>,
    #[allow(dead_code)]
    persistence_path: Option<PathBuf>,
}

/// Native: common setup with PathBuf for command-line ROM loading
fn common_setup(rom: Option<PathBuf>) -> SetupResponse {
    // Create the emulator instance
    let console = Nes::default();

    // Create channel-based emulator wrapper
    let (emu, to_emu, from_emu) = ChannelEmulator::new(console);
    let (async_sender, from_async) = crossbeam_channel::unbounded();

    if rom.is_some() {
        // Setup Emulator State via messages - read ROM file if provided
        let loaded_rom = rom.as_ref().and_then(|path| {
            let data = std::fs::read(path).ok()?;
            let name = path.file_name()?.to_string_lossy().to_string();
            let directory = path
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .map(|f| StorageKey::from(&f))
                .unwrap();

            Some(LoadedRom {
                data,
                name,
                directory,
            })
        });
        let _ = async_sender.send(AsyncFrontendMessage::LoadRom(loaded_rom));
    } else {
        let _ = async_sender.send(AsyncFrontendMessage::LoadRom(None));
    }

    let _ = to_emu.send(FrontendMessage::AttachPeripherals((
        Some(ExpansionDevice::StandardController),
        Some(ExpansionDevice::StandardController),
    )));

    // Get the storage path for egui persistence
    let persistence_path = get_egui_storage_path();

    SetupResponse {
        persistence_path,
        emu,
        to_emu,
        from_emu,
        async_sender,
        from_async,
    }
}

/// Run the egui frontend.
///
/// Uses `RendererKind` for runtime-switchable rendering.
#[cfg(not(target_arch = "wasm32"))]
pub fn run(rom: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let res = common_setup(rom);
    run_internal(res)
}

/// Run the egui frontend for WASM.
#[cfg(target_arch = "wasm32")]
pub fn run(_: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let res = common_setup(None);
    run_internal_wasm(res)
}

#[tokio::main]
#[cfg(not(target_arch = "wasm32"))]
async fn run_internal(res: SetupResponse) -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration before starting eframe (we're in an async context)
    let loaded_config = load_config().await;

    // Configure eframe options
    // Disable vsync to allow uncapped frame rates - emulator handles its own timing
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0])
            .with_title("Monsoon")
            .with_app_id("monsoon-emulator"),
        vsync: false, // Disable vsync for uncapped performance
        // Enable persistence with custom storage path
        persistence_path: res.persistence_path,
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    // Run the application
    eframe::run_native(
        "Monsoon",
        options,
        Box::new(move |cc| {
            let style = Style {
                visuals: Visuals::dark(),
                ..Default::default()
            };
            cc.egui_ctx.set_global_style(style);
            cc.egui_ctx.set_theme(egui::Theme::Dark);
            Ok(Box::new(EguiApp::new(
                cc,
                loaded_config,
                res.emu,
                res.to_emu,
                res.from_emu,
                res.async_sender,
                res.from_async,
            )))
        }),
    )?;

    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn run_internal_wasm(res: SetupResponse) -> Result<(), Box<dyn std::error::Error>> {
    use eframe::web_sys;
    use wasm_bindgen::JsCast;
    use web_sys::HtmlCanvasElement;

    wasm_bindgen_futures::spawn_local(async {
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();

        let canvas = document
            .create_element("canvas")
            .unwrap()
            .dyn_into::<HtmlCanvasElement>()
            .unwrap();

        document.body().unwrap().append_child(&canvas).unwrap();

        // Load configuration before starting eframe (we're in an async context)
        let loaded_config = load_config().await;

        // Configure eframe options
        // Disable vsync to allow uncapped frame rates - emulator handles its own timing
        let options = eframe::WebOptions {
            ..Default::default()
        };

        eframe::WebRunner::new()
            .start(
                canvas,
                options,
                Box::new(move |cc| {
                    let style = Style {
                        visuals: Visuals::dark(),
                        ..Default::default()
                    };
                    cc.egui_ctx.set_style(style);
                    cc.egui_ctx.set_theme(egui::Theme::Dark);
                    Ok(Box::new(EguiApp::new(
                        cc,
                        loaded_config,
                        res.emu,
                        res.to_emu,
                        res.from_emu,
                        res.async_sender,
                        res.from_async,
                    )))
                }),
            )
            .await
            .unwrap()
    });

    Ok(())
}
