use std::cell::Cell;
use std::fmt::{Display, Formatter};
use std::ops::RangeInclusive;

use crate::emulation::mem::memory_map::MemoryMap;
use crate::emulation::mem::mirror_memory::MirrorMemory;
use crate::emulation::mem::palette_ram::PaletteRam;
use crate::emulation::mem::{Memory, OpenBus, Ram};
use crate::emulation::nes::ExecutionFinished;
// Re-import public constants/types from ppu_util so internal code can use them
// with short names.
pub use crate::emulation::ppu_util::{
    EmulatorFetchable, NAMETABLE_COLS, NAMETABLE_COUNT, NAMETABLE_ROWS, NametableData,
    PALETTE_RAM_START_ADDRESS, PaletteData, TILE_SIZE, TOTAL_OUTPUT_HEIGHT, TOTAL_OUTPUT_WIDTH,
    TileData,
};
use crate::emulation::ppu_util::{SPRITE_COUNT, SoamData, Sprite, SpriteData, SpriteMode};
use crate::emulation::rom::RomFile;
use crate::emulation::savestate::PpuState;

pub const PATTERN_TABLE_SIZE: usize = 256;
pub const VBLANK_NMI_BIT: u8 = 0x80;
pub const VRAM_ADDR_INC_BIT: u8 = 0x4;
pub const BACKGROUND_RENDER_BIT: u8 = 0x8;
pub const SPRITE_RENDER_BIT: u8 = 0x10;
pub const VRAM_ADDR_COARSE_X_SCROLL_MASK: u16 = 0x1F;
pub const VRAM_ADDR_COARSE_Y_SCROLL_MASK: u16 = 0x3E0;
pub const VRAM_ADDR_FINE_Y_SCROLL_MASK: u16 = 0x7000;
pub const PALETTE_RAM_END_INDEX: u16 = 0x3FFF;
pub const PALETTE_RAM_SIZE: u16 = 0x20;
pub const VRAM_SIZE: usize = 0x800;
pub const DOTS_PER_SCANLINE: u16 = 340;
pub const OPEN_BUS_DECAY_DELAY: u32 = 420_000;
pub const SPRITE_OVERFLOW_FLAG: u8 = 0b0010_0000;
pub const SPRITE_ZERO_FLAG: u8 = 0b0100_0000;
pub const BYTES_PER_TILE: usize = 16; // 8 low plane + 8 high plane
pub const TABLE_BYTES: usize = 0x1000; // 256 tiles * 16 bytes
pub const VBL_START_SCANLINE: u16 = 241;
pub const VISIBLE_SCANLINES: u16 = 239;
pub const PRE_RENDER_SCANLINE: u16 = 261;
pub const OAM_SIZE: usize = 0x100;
pub const NAMETABLE_TILE_AREA_SIZE: u16 = 0x3C0;
pub const NAMETABLE_SIZE: u16 = 0x400;
pub const ATTRIBUTE_TABLE_BASE_ADDRESS: u16 = 0x23C0;

pub const SCREEN_RENDER_WIDTH: usize = 256;

pub struct Ppu {
    pub dot_counter: u128,
    pub ctrl_register: u8,
    pub mask_register: u8,
    pub status_register: Cell<u8>,
    pub oam_addr_register: u8,
    pub v_register: u16,
    pub ppu_data_buffer: u8,
    pub nmi_requested: Cell<bool>,
    pub memory: MemoryMap,
    pub palette_ram: MemoryMap,
    pub oam: MemoryMap,
    pub write_latch: Cell<bool>,
    pub t_register: u16,
    pub fine_x_scroll: u8,
    pub even_frame: bool,
    pub reset_signal: bool,
    pub pixel_buffer: Vec<u16>,
    pub vbl_reset_counter: Cell<u8>,
    pub vbl_clear_scheduled: Cell<Option<u8>>,
    pub scanline: u16,
    pub dot: u16,
    pub prev_vbl: u8,
    pub open_bus: Cell<OpenBus>,
    pub address_bus: u16,
    pub address_latch: u8,
    pub shift_pattern_lo: u16,
    pub shift_pattern_hi: u16,
    pub shift_attr_lo: u8,
    pub shift_attr_hi: u8,
    pub shift_in_attr_lo: bool,
    pub shift_in_attr_hi: bool,
    pub bg_next_tile_id: u8,
    pub bg_next_tile_attribute: u8,
    pub bg_next_tile_lsb: u8,
    pub is_soam_clear_active: bool,
    pub oam_index: u8,
    pub soam_index: u8,
    pub soam_disable: bool,
    pub oam_increment: u8,
    pub soam_write_counter: u8,
    pub current_sprite_y: u8,
    pub sprite_fifos: [SpriteFifo; 8],
    pub current_sprite_tile_id: u8,
    pub oam_fetch: u8,
    pub log: String,
}

impl Default for Ppu {
    fn default() -> Self { Self::new() }
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            dot_counter: 0,
            ctrl_register: 0,
            mask_register: 0,
            status_register: 0.into(),
            oam_addr_register: 0,
            v_register: 0,
            ppu_data_buffer: 0,
            bg_next_tile_id: 0,
            bg_next_tile_attribute: 0,
            bg_next_tile_lsb: 0,
            fine_x_scroll: 0,
            nmi_requested: Cell::new(false),
            memory: Self::get_default_memory_map(),
            palette_ram: Self::get_palette_memory_map(),
            oam: Self::get_default_oam(),
            write_latch: false.into(),
            t_register: 0,
            even_frame: false,
            reset_signal: false,
            pixel_buffer: vec![0; TOTAL_OUTPUT_HEIGHT * TOTAL_OUTPUT_WIDTH],
            vbl_reset_counter: 0.into(),
            vbl_clear_scheduled: None.into(),
            scanline: 0,
            dot: 0,
            prev_vbl: 0,
            open_bus: OpenBus::new(OPEN_BUS_DECAY_DELAY).into(),
            address_bus: 0,
            address_latch: 0,
            shift_pattern_lo: 0,
            shift_pattern_hi: 0,
            shift_attr_lo: 0,
            shift_attr_hi: 0,
            shift_in_attr_lo: false,
            shift_in_attr_hi: false,
            is_soam_clear_active: false,
            oam_index: 0,
            soam_index: 0,
            soam_disable: false,
            oam_increment: 4,
            soam_write_counter: 0,
            current_sprite_y: 0,
            sprite_fifos: [SpriteFifo::default(); 8],
            current_sprite_tile_id: 0,
            oam_fetch: 0,
            log: "".to_string(),
        }
    }

    fn get_default_memory_map() -> MemoryMap { MemoryMap::default() }

    fn get_palette_memory_map() -> MemoryMap {
        let mut mem = MemoryMap::default();

        mem.add_memory(
            0..=PALETTE_RAM_END_INDEX - PALETTE_RAM_START_ADDRESS,
            Memory::MirrorMemory(MirrorMemory::new(
                Box::new(Memory::PaletteRam(PaletteRam::default())),
                PALETTE_RAM_SIZE - 1,
            )),
        );

        mem
    }

    fn get_default_oam() -> MemoryMap {
        let mut map = MemoryMap::new();
        map.add_memory(
            0..=((OAM_SIZE as u16) + OAM_SIZE as u16 / 8u16),
            Memory::Ram(Ram::new(OAM_SIZE + OAM_SIZE / 8)),
        );

        map
    }

    #[inline(always)]
    pub fn step(&mut self) -> ExecutionFinished {
        self.prev_vbl = self.status_register.get() & VBLANK_NMI_BIT;

        if self.reset_signal {
            self.ctrl_register = 0;
            self.mask_register = 0;
            self.write_latch.set(false);
            self.ppu_data_buffer = 0;
            self.t_register = 0;
        }

        let is_frame_start = self.dot == 0 && self.scanline == 0;

        if is_frame_start {
            self.even_frame = !self.even_frame;
        }

        if (self.scanline < VISIBLE_SCANLINES + 1 || self.scanline == PRE_RENDER_SCANLINE)
            && self.is_rendering()
        {
            if self.dot == 0 {
                self.soam_index = 0;
                self.oam_index = 0;
                self.current_sprite_y = 0;
                self.oam_increment = 4;
                self.soam_write_counter = 0;
                self.set_soam_disable(false);
            }

            if self.scanline == PRE_RENDER_SCANLINE {
                self.set_soam_disable(true);
                self.clear_sprite_overflow();
                self.clear_sprite_zero()
            }

            // Replace (1..=64).contains(&self.dot) with manual comparison
            if self.dot >= 1 && self.dot <= 64 {
                self.set_soam_disable(false);
                self.is_soam_clear_active = true;
                self.init_soam();
            } else {
                self.is_soam_clear_active = false;
            }

            // Replace (65..=256).contains(&self.dot) with manual comparison
            if self.dot >= 65 && self.dot <= 256 {
                self.sprite_eval();
            }

            // Replace (257..=320).contains(&self.dot) with manual comparison
            if self.dot >= 257 && self.dot <= 320 {
                if self.dot == 257 {
                    self.soam_index = 0;
                }

                self.sprite_fetch();
            }

            // Replace (321..=341).contains(&self.dot) with manual comparison
            if self.dot >= 321 && self.dot <= 341 {
                if self.dot.is_multiple_of(2) {
                    self.secondary_oam_read(self.soam_index);
                }

                if self.dot == 339 {
                    for s in self.sprite_fifos.iter_mut() {
                        s.is_counting = true;
                    }
                }
            }

            // Replace (1..=256).contains(&self.dot) || (321..=336).contains(&self.dot) with manual comparison
            if (self.dot >= 1 && self.dot <= 256) || (self.dot >= 321 && self.dot <= 336) {
                self.do_dot_fetch();

                if self.scanline != PRE_RENDER_SCANLINE && self.dot >= 0x01 && self.dot <= 256 {
                    let bg_color_address = self.get_bg_pixel();

                    let mut sprite_pixel_palette = 0u8;
                    let mut sprite_pixel_pattern = 0u8;
                    let mut sprite_pixel_priority = 0;
                    let mut sprite_zero_outputting = false;

                    for (i, s) in self.sprite_fifos.iter_mut().enumerate() {
                        if s.down_counter == 0 && !s.is_counting {
                            let shift_out_lo = (s.shifter_pattern_lo & 0x80 != 0) as u8;
                            let shift_out_hi = (s.shifter_pattern_hi & 0x80 != 0) as u8;

                            s.shifter_pattern_lo <<= 1;
                            s.shifter_pattern_hi <<= 1;

                            let pattern = (shift_out_hi << 1) | shift_out_lo;

                            if pattern != 0 && sprite_pixel_pattern == 0 {
                                sprite_pixel_priority = s.attribute & 0b0010_0000;
                                sprite_pixel_palette = s.attribute & 3;
                                sprite_pixel_pattern = pattern;

                                if i == 0 {
                                    sprite_zero_outputting = true;
                                }
                                // Early exit: we found the first non-transparent sprite
                                break;
                            }
                        }
                    }

                    let sprite_color_address = if sprite_pixel_pattern == 0 {
                        0x3F10
                    } else {
                        0x3F10
                            + ((sprite_pixel_palette as u16) << 2)
                            + (sprite_pixel_pattern as u16)
                    };

                    let pixel_color_address = if bg_color_address == PALETTE_RAM_START_ADDRESS
                        && sprite_color_address == 0x3F10
                    {
                        PALETTE_RAM_START_ADDRESS
                    } else if bg_color_address == PALETTE_RAM_START_ADDRESS
                        && sprite_color_address != 0x3F10
                    {
                        sprite_color_address
                    } else if bg_color_address != PALETTE_RAM_START_ADDRESS
                        && sprite_color_address == 0x3F10
                    {
                        bg_color_address
                    } else if sprite_pixel_priority == 0 {
                        sprite_color_address
                    } else {
                        bg_color_address
                    };

                    let pixel_color = self.mem_read(pixel_color_address);

                    if sprite_color_address != 0x3F10
                        && bg_color_address != PALETTE_RAM_START_ADDRESS
                        && sprite_zero_outputting
                    {
                        self.set_sprite_zero();
                    }

                    self.pixel_buffer
                        [self.scanline as usize * SCREEN_RENDER_WIDTH + (self.dot - 1) as usize] =
                        (pixel_color as u16) | ((self.get_emph_bits() as u16) << 6);
                }

                for s in self.sprite_fifos.iter_mut() {
                    if s.is_counting && s.down_counter > 0 {
                        s.down_counter -= 1;

                        if s.down_counter == 0 {
                            s.is_counting = false;
                        }
                    }
                }

                self.shift_bg_shifters();

                if (self.dot - 1) % 8 == 7 {
                    self.inc_coarse_x_scroll();
                    self.reload_shifters();
                }

                if self.dot == 256 {
                    self.inc_y_scroll()
                }
            }

            // Replace (257..=320).contains(&self.dot) with manual comparison
            if self.dot >= 257 && self.dot <= 320 {
                if self.dot == 257 {
                    self.v_register = (self.v_register & !0x041F) | (self.t_register & 0x041F);
                }

                self.oam_addr_register = 0;
            }

            // Replace (280..=304).contains(&self.dot) with manual comparison
            if self.dot >= 280 && self.dot <= 304 && self.scanline == PRE_RENDER_SCANLINE {
                self.v_register = (self.v_register & !0x7BE0) | (self.t_register & 0x7BE0);
            }
        }

        if self.scanline == VBL_START_SCANLINE && self.dot == 1 {
            self.set_vbl_bit();
        }

        self.process_vbl_clear_scheduled();

        if self.scanline == PRE_RENDER_SCANLINE && self.dot == 1 {
            self.clear_vbl_bit();
            self.reset_signal = false;
        }

        self.update_nmi();

        // Skip cycle on odd frames when rendering (skips from dot 339 directly to next frame)
        // This occurs at dot 339 of scanline 261 on odd frames when rendering is enabled
        let skip_cycle = self.dot == DOTS_PER_SCANLINE - 1
            && self.scanline == PRE_RENDER_SCANLINE
            && !self.even_frame
            && self.is_rendering();

        if skip_cycle {
            self.dot_counter += 1;
        }

        self.address_latch = self.mem_read(self.address_bus);

        self.dot_counter += 1;

        // Increment dot/scanline directly instead of recalculating from dot_counter
        // Handle skip_cycle by incrementing by 2 (skipping dot 340)
        let increment = if skip_cycle { 2 } else { 1 };
        self.dot += increment;

        let mut is_frame_end = false;
        let mut is_scanline_end = false;

        if self.dot > DOTS_PER_SCANLINE {
            self.dot = 0;
            self.scanline += 1;

            is_scanline_end = true;

            if self.scanline > PRE_RENDER_SCANLINE {
                self.scanline = 0;
                is_frame_end = true;
            }
        }

        ExecutionFinished {
            frame_done: is_frame_end,
            scanline_done: is_scanline_end,
            ..Default::default()
        }
    }

    #[inline]
    pub fn get_emph_bits(&self) -> u8 { self.get_mask_register() >> 5 }

    #[inline]
    pub fn sprite_fetch(&mut self) {
        match (self.dot - 1) % 8 {
            0 => {
                self.current_sprite_y = self.secondary_oam_read(self.soam_index);
            }
            1 => {
                self.get_sprite_fifo_for_soam_index().attribute =
                    self.secondary_oam_read(self.soam_index + 2);
            }
            2 => {
                self.current_sprite_tile_id = self.secondary_oam_read(self.soam_index + 1);
                let sprite_height = self.get_sprite_height();

                let table_base = if sprite_height == 8 {
                    ((self.ctrl_register & 0x8) as u16) << 9
                } else {
                    ((self.current_sprite_tile_id & 1) as u16) << 12
                };

                let raw_row = (self.scanline as u8).wrapping_sub(self.current_sprite_y);

                let row_offset = if self.get_sprite_fifo_for_soam_index().attribute & 0x80 == 0 {
                    raw_row
                } else {
                    (self.get_sprite_height() - 1).wrapping_sub(raw_row)
                };

                let tile_id = if sprite_height == 8 {
                    self.current_sprite_tile_id as u16
                } else {
                    // Strip bit 0 (used for table select), then select top or bottom tile
                    let base_tile = (self.current_sprite_tile_id & 0xFE) as u16;
                    if row_offset < 8 {
                        base_tile
                    } else {
                        base_tile + 1
                    }
                };

                let fine_y = row_offset % 8;

                self.address_bus = table_base + (tile_id * 16) + fine_y as u16;

                let mut pattern = self.mem_read(self.address_bus);

                if self.get_sprite_fifo_for_soam_index().attribute & 0b0100_0000 != 0 {
                    pattern = pattern.reverse_bits();
                }

                self.get_sprite_fifo_for_soam_index().shifter_pattern_lo = pattern;

                if !self.is_sprite_in_range() {
                    self.get_sprite_fifo_for_soam_index().shifter_pattern_lo = 0;
                }
            }
            3 => {
                self.get_sprite_fifo_for_soam_index().down_counter =
                    self.secondary_oam_read(self.soam_index + 3);

                let mut pattern = self.mem_read(self.address_bus + 8);

                if self.get_sprite_fifo_for_soam_index().attribute & 0b0100_0000 != 0 {
                    pattern = pattern.reverse_bits();
                }

                self.get_sprite_fifo_for_soam_index().shifter_pattern_hi = pattern;

                if !self.is_sprite_in_range() {
                    self.get_sprite_fifo_for_soam_index().shifter_pattern_hi = 0;
                }
            }
            4..8 => {
                self.get_sprite_fifo_for_soam_index().down_counter =
                    self.secondary_oam_read(self.soam_index + 3);
                if (self.dot - 1) % 8 == 7 {
                    self.soam_index += 4;
                }
            }
            _ => unreachable!(),
        }
    }

    fn get_sprite_fifo_for_soam_index(&mut self) -> &mut SpriteFifo {
        &mut self.sprite_fifos[(self.soam_index as usize / 4) % 8]
    }

    #[inline]
    pub fn is_sprite_in_range(&self) -> bool {
        let (diff, o) = (self.scanline as u8).overflowing_sub(self.current_sprite_y);

        !(o || diff >= self.get_sprite_height())
    }

    #[inline]
    pub fn sprite_eval(&mut self) {
        match (self.dot - 1).is_multiple_of(2) {
            true => {
                self.oam_addr_register = self.oam_index;
                self.oam_fetch = self.get_oam_at_addr();

                if self.soam_write_counter == 0 {
                    self.current_sprite_y = self.oam_fetch;
                }
            }
            false => {
                let write = if self.scanline != PRE_RENDER_SCANLINE {
                    self.oam_fetch
                } else {
                    self.secondary_oam_read(self.soam_index)
                };

                self.secondary_oam_write(self.soam_index, write);

                if self.is_sprite_in_range() {
                    self.oam_increment = 1;

                    self.soam_index += 1;

                    if self.soam_write_counter == 3 {
                        self.soam_write_counter = 0;
                    } else {
                        self.soam_write_counter += 1;
                    }

                    if self.scanline == PRE_RENDER_SCANLINE {
                        self.soam_write_counter = 0;
                    }

                    if self.soam_index > 32 {
                        self.set_sprite_overflow();
                    }
                } else {
                    self.soam_write_counter = 0;
                    self.oam_increment = 4;
                }

                if self.soam_index >= 32 {
                    self.oam_increment = 5;
                    self.set_soam_disable(true);
                }

                if self.scanline != PRE_RENDER_SCANLINE {
                    let (i, o) = self.oam_index.overflowing_add(self.oam_increment);
                    self.oam_index = i;

                    if o {
                        self.set_soam_disable(true);
                    }
                }
            }
        }
    }

    pub fn set_soam_disable(&mut self, disable: bool) { self.soam_disable = disable; }

    #[inline]
    pub fn init_soam(&mut self) {
        match (self.dot - 1).is_multiple_of(2) {
            true => {
                self.oam_addr_register = (self.dot - 1) as u8;
                self.oam_fetch = self.get_oam_at_addr();
            }
            false => {
                self.secondary_oam_write(((self.dot - 1) / 2) as u8, self.oam_fetch);
            }
        }
    }

    #[inline]
    pub fn shift_bg_shifters(&mut self) {
        self.shift_pattern_lo <<= 1;
        self.shift_pattern_hi <<= 1;
        self.shift_attr_lo <<= 1;
        self.shift_attr_lo |= self.shift_in_attr_lo as u8;
        self.shift_attr_hi <<= 1;
        self.shift_attr_hi |= self.shift_in_attr_hi as u8;
    }

    #[inline]
    pub fn get_bg_pixel(&mut self) -> u16 {
        let mux = 0x80 >> self.fine_x_scroll;
        // pattern shifters (16-bit)
        let bit0 = ((self.shift_pattern_lo & (mux as u16) << 8) != 0) as u8;
        let bit1 = ((self.shift_pattern_hi & (mux as u16) << 8) != 0) as u8;

        // attribute shifters (8-bit)
        let attr0 = ((self.shift_attr_lo & mux) != 0) as u8;
        let attr1 = ((self.shift_attr_hi & mux) != 0) as u8;

        let pattern_index = (bit1 << 1) | bit0;
        let palette_index = (attr1 << 1) | attr0;

        if pattern_index == 0 {
            PALETTE_RAM_START_ADDRESS
        } else {
            PALETTE_RAM_START_ADDRESS + ((palette_index as u16) << 2) + (pattern_index as u16)
        }
    }

    #[inline]
    pub fn do_dot_fetch(&mut self) {
        match (self.dot - 1) % 8 {
            0 => {
                self.address_bus = 0x2000 | (self.v_register & 0x0FFF);
            }
            1 => {
                self.bg_next_tile_id = self.address_latch;
            }
            2 => {
                self.address_bus = ATTRIBUTE_TABLE_BASE_ADDRESS
                    | (self.v_register & 0x0C00)
                    | ((self.v_register >> 4) & 0x38)
                    | ((self.v_register >> 2) & 0x7);
            }
            3 => {
                let coarse_x = self.get_coarse_x_scroll();
                let coarse_y = self.get_coarse_y_scroll() >> 5;

                let attr_byte = self.address_latch;
                let shift = ((coarse_y & 2) << 1) | (coarse_x & 2);
                let palette_bits = (attr_byte >> shift) & 0b11;
                self.bg_next_tile_attribute = palette_bits;
            }
            4 => {
                let fine_y = self.get_fine_y_scroll();

                let table_base = if self.ctrl_register & 0x10 != 0 {
                    0x1000
                } else {
                    0x0000
                };

                self.address_bus =
                    table_base + ((self.bg_next_tile_id as u16) * 16) + fine_y as u16;
            }
            5 => {
                self.bg_next_tile_lsb = self.address_latch;
            }
            6 => {
                let fine_y = self.get_fine_y_scroll();
                let table_base = if self.ctrl_register & 0x10 != 0 {
                    0x1000
                } else {
                    0x0000
                };

                self.address_bus =
                    table_base + ((self.bg_next_tile_id as u16) * 16) + (fine_y + 8) as u16;
            }
            7 => {}
            _ => unreachable!(),
        }
    }

    #[inline]
    pub fn is_rendering(&self) -> bool {
        self.is_background_rendering() || self.is_sprite_rendering()
    }

    #[inline]
    pub fn get_sprite_height(&self) -> u8 {
        if self.ctrl_register & 0x20 == 0 {
            8
        } else {
            16
        }
    }

    #[inline]
    pub fn set_sprite_overflow(&self) {
        self.status_register
            .set(self.status_register.get() | SPRITE_OVERFLOW_FLAG);
    }

    #[inline]
    pub fn clear_sprite_overflow(&self) {
        self.status_register
            .set(self.status_register.get() & !SPRITE_OVERFLOW_FLAG);
    }

    #[inline]
    pub fn set_sprite_zero(&self) {
        self.status_register
            .set(self.status_register.get() | SPRITE_ZERO_FLAG);
    }

    #[inline]
    pub fn clear_sprite_zero(&self) {
        self.status_register
            .set(self.status_register.get() & !SPRITE_ZERO_FLAG);
    }

    #[inline]
    pub fn get_vram_addr_step(&self) -> u8 {
        if self.ctrl_register & VRAM_ADDR_INC_BIT == 0 {
            1
        } else {
            32
        }
    }

    #[inline]
    pub fn update_nmi(&self) {
        if (self.status_register.get() & self.ctrl_register & VBLANK_NMI_BIT) != 0 {
            self.nmi_requested.set(true);
        } else {
            self.nmi_requested.set(false);
        }
    }

    #[inline]
    pub fn clear_vbl_bit(&self) {
        self.status_register
            .set(self.status_register.get() & !VBLANK_NMI_BIT);
        self.update_nmi()
    }

    #[inline]
    pub fn set_vbl_bit(&mut self) {
        self.status_register
            .set(self.status_register.get() | VBLANK_NMI_BIT);
        self.update_nmi()
    }

    #[inline]
    pub fn get_ppu_status(&self) -> u8 {
        let result = (self.status_register.get() & !VBLANK_NMI_BIT) | self.prev_vbl;
        self.vbl_clear_scheduled.set(Some(2));
        self.process_vbl_clear_scheduled();

        self.write_latch.set(false);
        result
    }

    #[inline]
    pub fn get_ppu_ctrl(&self) -> u8 { self.ctrl_register }

    #[inline]
    pub fn set_ppu_ctrl(&mut self, value: u8) {
        if !self.reset_signal {
            self.ctrl_register = value;

            self.t_register = (self.t_register & 0xF3FF) | (((value as u16) & 0x03) << 10)
        }

        self.update_nmi();
    }

    #[inline]
    pub fn get_mask_register(&self) -> u8 { self.mask_register }

    #[inline]
    pub fn set_mask_register(&mut self, value: u8) {
        if !self.reset_signal {
            self.mask_register = value;
        }
    }

    #[inline]
    pub fn set_oam_addr_register(&mut self, value: u8) { self.oam_addr_register = value }

    #[inline]
    pub fn get_oam_at_addr(&mut self) -> u8 {
        if self.is_soam_clear_active {
            0xFF
        } else {
            self.oam_read(self.oam_addr_register)
        }
    }

    #[inline]
    pub fn get_vram_at_addr(&mut self) -> u8 {
        let mut ret = self.ppu_data_buffer;

        if !(PALETTE_RAM_START_ADDRESS..=PALETTE_RAM_END_INDEX).contains(&self.v_register) {
            self.ppu_data_buffer = self.mem_read(self.v_register);
        } else {
            ret = self.mem_read(self.v_register);
            self.ppu_data_buffer = self.mem_read(self.v_register - 0x1000);
        }

        if !(self.scanline < VISIBLE_SCANLINES + 1 || self.scanline == PRE_RENDER_SCANLINE)
            || !self.is_rendering()
        {
            self.v_register = self
                .v_register
                .wrapping_add(self.get_vram_addr_step() as u16);
        }

        ret
    }

    #[inline]
    pub fn write_oam(&mut self, mut data: u8) {
        if self.oam_addr_register % 4 == 2 {
            data &= 0xE3;
        }

        if !(self.scanline < VISIBLE_SCANLINES + 1 || self.scanline == PRE_RENDER_SCANLINE)
            || !self.is_rendering()
        {
            self.oam_write(self.oam_addr_register, data);
            self.oam_addr_register = self.oam_addr_register.wrapping_add(1);
        }
    }

    #[inline]
    pub fn write_vram(&mut self, data: u8) {
        self.mem_write(self.v_register, data);
        self.v_register = self
            .v_register
            .wrapping_add(self.get_vram_addr_step() as u16);
    }

    #[inline]
    pub fn write_ppu_scroll(&mut self, data: u8) {
        if self.reset_signal {
            return;
        }

        if !self.write_latch.get() {
            // First write to $2005 (horizontal)
            // coarse X = bits 3–7, fine X = bits 0–2
            self.t_register = (self.t_register & !0b1_1111) | ((data >> 3) as u16);
            self.fine_x_scroll = data & 0x07;
        } else {
            // Second write to $2005 (vertical)
            // coarse Y = bits 3–7 → t[5–9]
            // fine Y = bits 0–2 → t[12–14]
            self.t_register = (self.t_register & !((0b1_1111u16 << 5) | (0x7 << 12)))
                | ((data as u16 & 0b1111_1000) << 2) // bits 3–7 → bits 5–9
                | ((data as u16 & !0b1111_1000) << 12); // bits 0–2 → bits 12–14
        }

        self.write_latch.set(!self.write_latch.get());
    }

    #[inline]
    pub fn write_vram_addr(&mut self, data: u8) {
        if !self.write_latch.get() {
            // First write: upper byte (but only lower 6 bits valid)
            self.t_register = (self.t_register & 0x00FF) | ((data as u16 & 0b00111111) << 8);
        } else {
            // Second write: lower byte
            self.t_register = (self.t_register & 0xFF00) | data as u16;
            self.v_register = self.t_register;
        }

        self.write_latch.set(!self.write_latch.get());
    }

    #[inline]
    pub fn poll_nmi(&self) -> bool { self.nmi_requested.get() }

    #[inline]
    pub fn is_background_rendering(&self) -> bool {
        self.mask_register & BACKGROUND_RENDER_BIT != 0
    }

    #[inline]
    pub fn is_sprite_rendering(&self) -> bool { self.mask_register & SPRITE_RENDER_BIT != 0 }

    #[inline]
    pub fn get_coarse_x_scroll(&self) -> u8 {
        (self.v_register & VRAM_ADDR_COARSE_X_SCROLL_MASK) as u8
    }

    #[inline]
    pub fn get_coarse_y_scroll(&self) -> u8 {
        (self.v_register & VRAM_ADDR_COARSE_Y_SCROLL_MASK) as u8
    }

    #[inline]
    pub fn get_fine_y_scroll(&self) -> u8 {
        ((self.v_register & VRAM_ADDR_FINE_Y_SCROLL_MASK) >> 12) as u8
    }

    #[inline]
    pub fn inc_coarse_x_scroll(&mut self) {
        if self.get_coarse_x_scroll() == 31 {
            self.v_register &= !VRAM_ADDR_COARSE_X_SCROLL_MASK;
            self.v_register ^= NAMETABLE_SIZE;
        } else {
            self.v_register += 1;
        }
    }

    #[inline]
    pub fn inc_y_scroll(&mut self) {
        if (self.v_register & VRAM_ADDR_FINE_Y_SCROLL_MASK) != VRAM_ADDR_FINE_Y_SCROLL_MASK {
            self.v_register += 0x1000;
        } else {
            self.v_register &= !VRAM_ADDR_FINE_Y_SCROLL_MASK;
            let mut y = ((self.v_register & 0x03E0) >> 5) as u8;
            if y == 29 {
                y = 0;
                self.v_register ^= VRAM_SIZE as u16;
            } else if y == 31 {
                y = 0;
            } else {
                y += 1;
            }

            self.v_register = (self.v_register & !0x03E0) | ((y as u16) << 5);
        }
    }

    pub fn mem_read_debug(&self, addr: u16) -> u8 {
        match addr {
            PALETTE_RAM_START_ADDRESS..PALETTE_RAM_END_INDEX => self
                .palette_ram
                .mem_read_debug(addr - PALETTE_RAM_START_ADDRESS),
            _ => self.memory.mem_read_debug(addr),
        }
    }

    #[inline]
    pub fn mem_read(&mut self, addr: u16) -> u8 {
        match addr {
            PALETTE_RAM_START_ADDRESS..PALETTE_RAM_END_INDEX => {
                self.palette_ram.mem_read(addr - PALETTE_RAM_START_ADDRESS)
            }
            _ => self.memory.mem_read(addr),
        }
    }

    #[inline]
    pub fn mem_write(&mut self, addr: u16, data: u8) {
        match addr {
            PALETTE_RAM_START_ADDRESS..PALETTE_RAM_END_INDEX => self
                .palette_ram
                .mem_write(addr - PALETTE_RAM_START_ADDRESS, data),
            _ => self.memory.mem_write(addr, data),
        }
    }

    #[inline]
    pub fn mem_init(&mut self, addr: u16, data: u8) {
        match addr {
            PALETTE_RAM_START_ADDRESS..PALETTE_RAM_END_INDEX => self
                .palette_ram
                .init(addr - PALETTE_RAM_START_ADDRESS, data),
            _ => self.memory.init(addr, data),
        }
    }

    #[inline]
    pub fn oam_read(&mut self, addr: u8) -> u8 {
        let row = addr / 8;
        let byte = addr % 8;
        let mut res = self.oam.mem_read((row as u16 * 9) + byte as u16);

        if self.is_soam_clear_active {
            res = 0xFF;
        }

        res
    }

    #[inline]
    pub fn oam_snapshot(&self, addr: u8) -> u8 {
        let row = addr / 8;
        let byte = addr % 8;
        let mut res = self.oam.mem_read_debug((row as u16 * 9) + byte as u16);

        if self.is_soam_clear_active {
            res = 0xFF;
        }

        res
    }

    #[inline]
    pub fn oam_write(&mut self, addr: u8, data: u8) {
        let row = addr / 8;
        let byte = addr % 8;
        self.oam.mem_write((row as u16 * 9) + byte as u16, data)
    }

    #[inline]
    pub fn secondary_oam_read(&mut self, addr: u8) -> u8 {
        // Mask for only 0-32
        let row = addr & 0x1F;
        let byte = 8u8;

        self.oam.mem_read((row as u16 * 9) + byte as u16)
    }

    #[inline]
    pub fn secondary_oam_snapshot(&self, addr: u8) -> u8 {
        let row = addr & 0x1F;
        let byte = 8u8;

        self.oam.mem_read_debug((row as u16 * 9) + byte as u16)
    }

    #[inline]
    pub fn secondary_oam_write(&mut self, addr: u8, data: u8) {
        // Mask for only 0-32
        let row = addr & 0x1F;
        let byte = 8u8;

        if !self.soam_disable {
            self.oam.mem_write((row as u16 * 9) + byte as u16, data);
        } else {
            self.oam.mem_read((row as u16 * 9) + byte as u16);
        }
    }

    pub fn load_rom(&mut self, rom_file: &RomFile) {
        let chr_rom = rom_file.get_chr_rom();

        if let Some(chr_rom) = chr_rom {
            self.memory.add_memory(0x0000..=0x1FFF, chr_rom);
        }

        self.memory.add_memory(
            0x2000..=PALETTE_RAM_END_INDEX,
            Memory::MirrorMemory(MirrorMemory::new(
                Box::new(rom_file.get_nametable_memory()),
                (VRAM_SIZE * 2 - 1) as u16,
            )),
        )
    }

    pub fn reset(&mut self) { self.reset_signal = false; }

    #[inline]
    pub fn get_pixel_buffer(&self) -> &Vec<u16> { &self.pixel_buffer }

    pub fn get_memory_debug(&self, range: Option<RangeInclusive<u16>>) -> Vec<u8> {
        self.memory.get_memory_debug(range)
    }

    #[inline]
    pub fn process_vbl_clear_scheduled(&self) {
        if let Some(vbl_clear_cycle) = self.vbl_clear_scheduled.get() {
            if vbl_clear_cycle >= self.vbl_reset_counter.get() {
                self.clear_vbl_bit();
            }

            if vbl_clear_cycle <= self.vbl_reset_counter.get() {
                self.vbl_clear_scheduled.set(None);
                self.vbl_reset_counter.set(0);
            }
        }
    }

    #[inline]
    fn reload_shifters(&mut self) {
        self.shift_pattern_lo = (self.shift_pattern_lo & 0xFF00) | self.bg_next_tile_lsb as u16;
        self.shift_pattern_hi = (self.shift_pattern_hi & 0xFF00) | self.address_latch as u16;

        // Decode the attribute bits for this tile (2 bits)
        let attr_low_bit = self.bg_next_tile_attribute & 0b01 != 0;
        let attr_high_bit = self.bg_next_tile_attribute & 0b10 != 0;

        // Load them into 8-bit shifters
        self.shift_in_attr_lo = attr_low_bit;
        self.shift_in_attr_hi = attr_high_bit;
    }

    fn load_palette_colors(&self) -> [[u8; 4]; 8] {
        let mut data = [[0; 4]; 8];

        for palette in 0..8 {
            let mut colors = [0; 4];
            let base = PALETTE_RAM_START_ADDRESS + palette * 4;

            for (i, color) in colors.iter_mut().enumerate() {
                let idx = self.mem_read_debug(base + i as u16);
                *color = idx;
            }

            data[palette as usize] = colors;
        }

        data
    }

    #[inline]
    pub fn tick_open_bus(&self, times: u8) {
        let mut bus = self.open_bus.get();
        bus.tick(times);
        self.open_bus.set(bus);
    }

    pub fn from(state: &PpuState, rom: &RomFile) -> Self {
        let mut ppu = Self {
            dot_counter: state.cycle_counter,
            ctrl_register: state.ctrl_register,
            mask_register: state.mask_register,
            status_register: state.status_register.into(),
            oam_addr_register: state.oam_addr_register,
            v_register: state.ppu_addr_register,
            ppu_data_buffer: state.ppu_data_buffer,
            nmi_requested: Cell::new(state.nmi_requested),
            memory: Self::get_default_memory_map(),
            palette_ram: Self::get_palette_memory_map(),
            oam: Self::get_default_oam(),
            write_latch: state.write_latch.into(),
            t_register: state.t_register,
            bg_next_tile_id: state.bg_next_tile_id,
            bg_next_tile_attribute: state.bg_next_tile_attribute,
            bg_next_tile_lsb: state.bg_next_tile_lsb,
            fine_x_scroll: state.fine_x_scroll,
            even_frame: state.even_frame,
            reset_signal: state.reset_signal,
            // Initialize pixel buffer fresh - it's not saved in savestate
            pixel_buffer: vec![0; TOTAL_OUTPUT_WIDTH * TOTAL_OUTPUT_HEIGHT],
            vbl_reset_counter: Cell::new(state.vbl_reset_counter),
            vbl_clear_scheduled: Cell::new(state.vbl_clear_scheduled),
            scanline: state.scanline,
            dot: state.dot,
            prev_vbl: state.prev_vbl,
            open_bus: Cell::new(state.open_bus),
            address_bus: state.address_bus,
            address_latch: state.address_latch,
            shift_pattern_lo: state.shift_pattern_lo,
            shift_pattern_hi: state.shift_pattern_hi,
            shift_attr_lo: state.shift_attr_lo,
            shift_attr_hi: state.shift_attr_hi,
            shift_in_attr_lo: state.shift_in_attr_lo,
            shift_in_attr_hi: state.shift_in_attr_hi,
            is_soam_clear_active: state.is_soam_clear_active,
            oam_index: state.oam_index,
            soam_index: state.soam_index,
            soam_disable: state.soam_disable,
            oam_increment: state.oam_increment,
            soam_write_counter: state.soam_write_counter, //1
            oam_fetch: state.oam_fetch,
            current_sprite_tile_id: 0,
            current_sprite_y: 0,
            sprite_fifos: [SpriteFifo::default(); 8],
            log: "".to_string(),
        };

        // Load ROM first to set up memory mapping (CHR ROM and nametables)
        ppu.load_rom(rom);

        // Restore nametable VRAM (at address 0x2000)
        ppu.memory.load_range(&state.nametable_ram, 0x2000);

        // Restore OAM and palette RAM
        ppu.oam.load(&state.oam_mem);
        ppu.palette_ram.load(&state.palette_ram);

        ppu
    }
}

impl Ppu {
    pub fn get_palettes_debug(&self) -> EmulatorFetchable {
        EmulatorFetchable::Palettes(Some(Box::new(PaletteData {
            colors: self.load_palette_colors(),
        })))
    }

    pub fn get_tiles_debug(&self) -> EmulatorFetchable {
        let mut tiles = [TileData::default(); 512];
        for table_ix in 0..2 {
            let table_base = (table_ix * TABLE_BYTES) as u16;
            for tile_index in 0..256 {
                let tile_addr = table_base + (tile_index * BYTES_PER_TILE) as u16;
                let mut plane_0: u64 = 0;
                let mut plane_1: u64 = 0;

                for y in 0..TILE_SIZE {
                    let p0 = self.mem_read_debug(tile_addr + y as u16);
                    let p1 = self.mem_read_debug(tile_addr + y as u16 + 8);

                    plane_0 = (plane_0 << 8) | p0 as u64;
                    plane_1 = (plane_1 << 8) | p1 as u64;
                }

                let tile: TileData = TileData {
                    address: tile_addr,
                    plane_0,
                    plane_1,
                };

                tiles[tile_index + table_ix * PATTERN_TABLE_SIZE] = tile;
            }
        }

        EmulatorFetchable::Tiles(Some(Box::new(tiles)))
    }

    pub fn get_nametable_debug(&self) -> EmulatorFetchable {
        let nametable_start = 0x2000u16;
        let pattern_table = ((self.ctrl_register & 0b1_0000) as u16) << 4;
        let mut nametables = [[0; NAMETABLE_ROWS * NAMETABLE_COLS]; NAMETABLE_COUNT];
        let mut attributes = [[0; 64]; NAMETABLE_COUNT];

        // Attribute table offset within each nametable (0x3C0 = 960 = 30 * 32)
        let attr_offset = NAMETABLE_TILE_AREA_SIZE;

        for nametable_index in 0..NAMETABLE_COUNT {
            let nametable_base = nametable_start + (nametable_index as u16) * NAMETABLE_SIZE;

            // Read tile indices
            for row in 0..NAMETABLE_ROWS {
                for col in 0..NAMETABLE_COLS {
                    let addr = nametable_base + (row * NAMETABLE_COLS + col) as u16;
                    let tile = (self.mem_read_debug(addr) as u16) | pattern_table;
                    nametables[nametable_index][row * NAMETABLE_COLS + col] = tile;
                }
            }

            // Read attribute table (64 bytes per nametable)
            let attr_base = nametable_base + attr_offset;
            for (i, attr) in attributes[nametable_index].iter_mut().enumerate() {
                *attr = self.mem_read_debug(attr_base + i as u16);
            }
        }

        EmulatorFetchable::Nametables(Some(Box::new(NametableData {
            tiles: nametables,
            palettes: attributes,
        })))
    }

    pub fn get_sprites_debug(&self) -> EmulatorFetchable {
        let sprite_mode = if self.get_sprite_height() == 8 {
            SpriteMode::SMALL
        } else {
            SpriteMode::TALL
        };

        let base_pattern_table = ((self.ctrl_register & 0b1000) as u16) << 5;

        let mut sprites = SpriteData {
            sprites: [Sprite::default(); 64],
            mode: sprite_mode,
        };

        for sprite in 0..SPRITE_COUNT {
            let sprite_base_address = sprite * 4;
            let y_pos = self.oam_snapshot(sprite_base_address as u8);
            let x_pos = self.oam_snapshot((sprite_base_address + 3) as u8);
            let tile_byte = self.oam_snapshot((sprite_base_address + 1) as u8);

            let tile = if sprite_mode == SpriteMode::SMALL {
                (tile_byte as u16) | base_pattern_table
            } else {
                ((tile_byte & !1) as u16) | (((tile_byte & 1) as u16) << 8)
            };

            let bottom_tile = if sprite_mode == SpriteMode::SMALL {
                0
            } else {
                ((tile_byte & !1) as u16 + 1) | (((tile_byte & 1) as u16) << 8)
            };

            let attribute_byte = self.oam_snapshot((sprite_base_address + 2) as u8);
            let priority = (attribute_byte << 2) >> 7 == 0;
            let h_flip = (attribute_byte << 1) >> 7 == 1;
            let v_flip = attribute_byte >> 7 == 1;

            let palette = (attribute_byte & 0b11) + 4;

            sprites.sprites[sprite] = Sprite {
                y_pos: y_pos as u16,
                x_pos: x_pos as u16,
                tile,
                bottom_tile,
                palette,
                priority,
                h_flip,
                v_flip,
            }
        }

        EmulatorFetchable::Sprites(Some(Box::new(sprites)))
    }

    pub fn get_soam_sprites_debug(&self) -> EmulatorFetchable {
        let sprite_mode = if self.get_sprite_height() == 8 {
            SpriteMode::SMALL
        } else {
            SpriteMode::TALL
        };

        let base_pattern_table = ((self.ctrl_register & 0b1000) as u16) << 5;

        let mut sprites = SoamData {
            sprites: [Sprite::default(); 8],
            mode: sprite_mode,
        };

        for sprite in 0..8 {
            let sprite_base_address = sprite * 4;
            let y_pos = self.secondary_oam_snapshot(sprite_base_address as u8);
            let x_pos = self.secondary_oam_snapshot((sprite_base_address + 3) as u8);
            let tile_byte = self.secondary_oam_snapshot((sprite_base_address + 1) as u8);

            let tile = if sprite_mode == SpriteMode::SMALL {
                (tile_byte as u16) | base_pattern_table
            } else {
                ((tile_byte & !1) as u16) | (((tile_byte & 1) as u16) << 8)
            };

            let bottom_tile = if sprite_mode == SpriteMode::SMALL {
                0
            } else {
                ((tile_byte & !1) as u16 + 1) | (((tile_byte & 1) as u16) << 8)
            };

            let attribute_byte = self.secondary_oam_snapshot((sprite_base_address + 2) as u8);
            let priority = (attribute_byte << 2) >> 7 == 0;
            let h_flip = (attribute_byte << 1) >> 7 == 1;
            let v_flip = attribute_byte >> 7 == 1;

            let palette = (attribute_byte & 0b11) + 4;

            sprites.sprites[sprite] = Sprite {
                y_pos: y_pos as u16,
                x_pos: x_pos as u16,
                tile,
                bottom_tile,
                palette,
                priority,
                h_flip,
                v_flip,
            }
        }

        EmulatorFetchable::SoamSprites(Some(Box::new(sprites)))
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct SpriteFifo {
    pub shifter_pattern_lo: u8,
    pub shifter_pattern_hi: u8,
    pub down_counter: u8,
    pub attribute: u8,
    pub is_counting: bool,
}

impl Display for SpriteFifo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut res = format!("Shifter Pattern Low:  {:08b}\n", self.shifter_pattern_lo);
        res += format!("Shifter Pattern High: {:08b}\n", self.shifter_pattern_hi).as_str();
        res += format!("Down Counter/X: {}\n", self.down_counter).as_str();
        res += format!("Attribute: {:08b}\n", self.attribute).as_str();
        f.write_str(res.as_str())
    }
}
