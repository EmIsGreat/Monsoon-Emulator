//! Reusable UI widgets for the emulator frontend.
//!
//! This module contains common widget patterns that are used across
//! multiple UI components to reduce code duplication.

use monsoon_core::emulation::palette_util::RgbColor;

/// Draw a colored cell with hover highlighting using RgbColor.
///
/// This is a common pattern used in palette viewers, pattern tables, etc.
/// It draws a filled rectangle and adds a white border when hovered.
///
/// # Arguments
/// * `ui` - The egui UI context
/// * `rect` - The rectangle to draw in
/// * `color` - The fill color as RgbColor tuple
/// * `sense` - The interaction sense (click, hover, etc.)
/// * `id_source` - A unique ID source for this widget (e.g., index in a grid)
///
/// # Returns
/// The response from the interaction
pub fn color_cell_rgb(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    color: RgbColor,
    sense: egui::Sense,
    id_source: impl std::hash::Hash,
) -> egui::Response {
    let response = ui.interact(rect, ui.id().with(id_source), sense);
    let painter = ui.painter();

    painter.rect_filled(
        rect,
        0.0,
        egui::Color32::from_rgb(color.r, color.g, color.b),
    );

    if response.hovered() {
        painter.rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(3.0, egui::Color32::WHITE),
            egui::StrokeKind::Inside,
        );
    }

    response
}

/// Draw an image cell (texture) with hover highlighting.
///
/// Similar to `color_cell` but for textures/images.
///
/// # Arguments
/// * `ui` - The egui UI context
/// * `rect` - The rectangle to draw in
/// * `texture_id` - The texture ID to draw
/// * `sense` - The interaction sense
/// * `id_source` - A unique ID source for this widget
///
/// # Returns
/// The response from the interaction
pub fn image_cell(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    texture_id: egui::TextureId,
    sense: egui::Sense,
    id_source: impl std::hash::Hash,
) -> egui::Response {
    image_cell_flipped(ui, rect, texture_id, false, false, sense, id_source)
}

/// Draw an image cell (texture) with optional horizontal/vertical flip.
pub fn image_cell_flipped(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    texture_id: egui::TextureId,
    h_flip: bool,
    v_flip: bool,
    sense: egui::Sense,
    id_source: impl std::hash::Hash,
) -> egui::Response {
    let response = ui.interact(rect, ui.id().with(id_source), sense);
    let painter = ui.painter();

    let uv = flip_uv(h_flip, v_flip);
    painter.image(texture_id, rect, uv, egui::Color32::WHITE);

    if response.hovered() {
        painter.rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(3.0, egui::Color32::WHITE),
            egui::StrokeKind::Inside,
        );
    }

    response
}

/// Draw two vertically stacked image cells with optional horizontal/vertical
/// flip.
///
/// When v_flip is set, each tile is flipped vertically and the top and bottom
/// tiles are swapped.
#[allow(clippy::too_many_arguments)]
pub fn image_cell_dual_vert_flipped(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    texture_id_1: egui::TextureId,
    texture_id_2: egui::TextureId,
    h_flip: bool,
    v_flip: bool,
    sense: egui::Sense,
    id_source: impl std::hash::Hash,
) -> egui::Response {
    let response = ui.interact(rect, ui.id().with(id_source), sense);
    let painter = ui.painter();
    let middle = rect.min.y + (rect.max.y - rect.min.y) / 2.0;

    let uv = flip_uv(h_flip, v_flip);

    // When vertically flipping, swap top and bottom tiles
    let (top_id, bottom_id) = if v_flip {
        (texture_id_2, texture_id_1)
    } else {
        (texture_id_1, texture_id_2)
    };

    painter.image(top_id, rect.with_max_y(middle), uv, egui::Color32::WHITE);
    painter.image(bottom_id, rect.with_min_y(middle), uv, egui::Color32::WHITE);

    if response.hovered() {
        painter.rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(3.0, egui::Color32::WHITE),
            egui::StrokeKind::Inside,
        );
    }

    response
}

/// Configuration for a grid layout drawn with the painter.
pub struct PainterGridConfig {
    /// Total width of the grid
    pub width: f32,
    /// Total height of the grid (if None, assumes square based on width)
    pub height: Option<f32>,
    /// Number of columns
    pub cols: usize,
    /// Number of rows
    pub rows: usize,
}

impl PainterGridConfig {
    /// Create a new square grid config
    pub fn square(width: f32, size: usize) -> Self {
        Self {
            width,
            height: None,
            cols: size,
            rows: size,
        }
    }

    /// Create a new rectangular grid config
    pub fn rect(width: f32, height: f32, cols: usize, rows: usize) -> Self {
        Self {
            width,
            height: Some(height),
            cols,
            rows,
        }
    }

    /// Get the height (defaults to width if not set)
    pub fn get_height(&self) -> f32 { self.height.unwrap_or(self.width) }

    /// Get the size of each cell
    pub fn cell_size(&self) -> egui::Vec2 {
        egui::vec2(
            self.width / self.cols as f32,
            self.get_height() / self.rows as f32,
        )
    }

    /// Get the rect for a cell at the given index
    pub fn cell_rect(&self, parent_min: egui::Pos2, index: usize) -> egui::Rect {
        let row = index / self.cols;
        let col = index % self.cols;
        let cell_size = self.cell_size();
        let min = parent_min + egui::vec2(col as f32 * cell_size.x, row as f32 * cell_size.y);
        egui::Rect::from_min_size(min, cell_size)
    }

    /// Total size of the grid
    pub fn total_size(&self) -> egui::Vec2 { egui::vec2(self.width, self.get_height()) }
}

/// Compute UV rect for a texture with optional horizontal/vertical flip.
fn flip_uv(h_flip: bool, v_flip: bool) -> egui::Rect {
    let u_min = if h_flip { 1.0 } else { 0.0 };
    let u_max = if h_flip { 0.0 } else { 1.0 };
    let v_min = if v_flip { 1.0 } else { 0.0 };
    let v_max = if v_flip { 0.0 } else { 1.0 };
    egui::Rect::from_min_max(egui::pos2(u_min, v_min), egui::pos2(u_max, v_max))
}
