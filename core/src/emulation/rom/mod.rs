//! ROM file parsing and metadata.
//!
//! This module provides types for loading and representing NES ROM files
//! in the iNES, NES 2.0, and related formats. The main entry point is
//! [`RomFile::load()`], which auto-detects the format and parses the ROM.
//!
//! For programmatic ROM construction (e.g., in tests), use [`RomBuilder`].

mod formats;

use std::error::Error;
use std::fmt::{Debug, Display, Formatter};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::emulation::mem::nametable_memory::{NametableArrangement, NametableMemory};
use crate::emulation::mem::{Memory, MemoryDevice, Ram, Rom};
use crate::emulation::rom::formats::archaic_ines::ArchaicInes;
use crate::emulation::rom::formats::ines::Ines;
use crate::emulation::rom::formats::ines_07::Ines07;
use crate::emulation::rom::formats::ines2::Ines2;

/// Errors that can occur while parsing a ROM file.
#[derive(Debug, Clone)]
pub enum ParseError {
    /// The sizes declared in the ROM header exceed the actual file length.
    SizeBiggerThanFile,
    /// The ROM data is too short to contain a valid header (minimum 16 bytes).
    InvalidHeader,
    /// The ROM format is not recognized (missing `NES\x1A` magic bytes).
    UnsupportedFormat,
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::SizeBiggerThanFile => {
                write!(
                    f,
                    "Rom sizes specified in header are larger than total rom size"
                )
            }
            ParseError::InvalidHeader => {
                write!(f, "Rom data is too short to contain a valid header")
            }
            ParseError::UnsupportedFormat => {
                write!(f, "Rom format is not recognized")
            }
        }
    }
}

impl Error for ParseError {}

/// Trait for ROM format parsers.
///
/// Each supported ROM format (iNES, NES 2.0, etc.) implements this trait.
/// Users should not need to call parsers directly; use [`RomFile::load()`]
/// instead, which auto-detects the format.
#[doc(hidden)]
pub trait RomParser: Debug {
    fn parse(&self, rom: &[u8], name: Option<String>) -> Result<RomFile, ParseError>;
}

/// A parsed NES ROM file.
///
/// Contains all metadata extracted from the ROM header (mapper number,
/// memory sizes, mirroring, etc.) as well as the raw ROM data used for
/// loading into the emulator's memory map.
///
/// # Loading a ROM
///
/// ```rust,no_run
/// use monsoon_core::emulation::rom::RomFile;
///
/// # let raw_bytes: &[u8] = &[];
/// let rom = RomFile::load(raw_bytes, Some("my_game.nes".to_string())).expect("invalid ROM");
/// println!("Mapper: {}", rom.mapper_number);
/// ```
///
/// # Constructing a ROM programmatically
///
/// Use [`RomBuilder`] for test scenarios where you need custom ROM metadata
/// without providing actual ROM data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct RomFile {
    /// Human-readable name of the ROM (typically the file name).
    pub name: Option<String>,
    /// PRG (program) memory sizes.
    pub prg_memory: PrgMemory,
    /// CHR (character/graphics) memory sizes.
    pub chr_memory: ChrMemory,
    /// iNES mapper number identifying the cartridge board hardware.
    pub mapper_number: u16,
    /// Default expansion device identifier (NES 2.0).
    pub default_expansion_device: u8,
    /// Number of miscellaneous ROM areas (NES 2.0).
    pub misc_rom_count: u8,
    /// Extended console type (NES 2.0), if applicable.
    pub extended_console_type: Option<u8>,
    /// VS System hardware type, if applicable.
    pub vs_system_hardware_type: Option<u8>,
    /// VS System PPU type, if applicable.
    pub vs_system_ppu_type: Option<u8>,
    /// CPU/PPU timing mode (0 = NTSC, 1 = PAL, 2 = Multi-region, 3 = Dendy).
    pub cpu_ppu_timing: u8,
    /// Console type (0 = NES/Famicom, 1 = VS System, 2 = Playchoice-10, 3 = Extended).
    pub console_type: u8,
    /// Nametable mirroring mode from header bit 0 (`true` = vertical, `false` = horizontal).
    pub hardwired_nametable_layout: bool,
    /// Whether the cartridge contains battery-backed persistent memory.
    pub is_battery_backed: bool,
    /// Whether a 512-byte trainer is present before PRG data.
    pub trainer_present: bool,
    /// Whether the ROM uses alternative nametable layouts.
    pub alternative_nametables: bool,
    /// Submapper number (NES 2.0).
    pub submapper_number: u8,
    /// SHA-256 checksum of the raw ROM data.
    pub data_checksum: [u8; 32],
    /// Raw ROM file bytes. Skipped during serialization to reduce save state size.
    #[serde(skip)]
    pub data: Vec<u8>,
}

/// PRG (program) memory size information from the ROM header.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PrgMemory {
    /// Size of PRG ROM in bytes.
    pub prg_rom_size: u32,
    /// Size of PRG RAM (volatile) in bytes.
    pub prg_ram_size: u32,
    /// Size of PRG NVRAM (non-volatile / battery-backed) in bytes.
    pub prg_nvram_size: u32,
}

impl PrgMemory {
    fn new(prg_rom_size: u32, prg_ram_size: u32, prg_nvram_size: u32) -> PrgMemory {
        Self {
            prg_rom_size,
            prg_nvram_size,
            prg_ram_size,
        }
    }
}

/// CHR (character/graphics) memory size information from the ROM header.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ChrMemory {
    /// Size of CHR ROM in bytes.
    pub chr_rom_size: u32,
    /// Size of CHR RAM (volatile) in bytes.
    pub chr_ram_size: u32,
    /// Size of CHR NVRAM (non-volatile) in bytes.
    pub chr_nvram_size: u32,
}

impl ChrMemory {
    fn new(chr_rom_size: u32, chr_ram_size: u32, chr_nvram_size: u32) -> ChrMemory {
        Self {
            chr_rom_size,
            chr_nvram_size,
            chr_ram_size,
        }
    }
}

impl RomFile {
    fn range_all_zeros(arr: &[u8], start: usize, end: usize) -> bool {
        if start > end || end > arr.len() {
            return false;
        }
        arr[start..end].iter().all(|&x| x == 0)
    }

    fn get_rom_type(rom: &[u8]) -> Result<Box<dyn RomParser>, ParseError> {
        // iNES and NES 2.0 headers are 16 bytes minimum
        if rom.len() < 16 {
            return Err(ParseError::InvalidHeader);
        }

        if rom.starts_with(&[0x4E, 0x45, 0x53, 0x1A]) {
            let prg_rom_size_lsb = rom[4] as u16;
            let prg_rom_size_msb = (rom[9] & 0xF) as u16;

            let prg_rom_size = Ines2::get_prg_rom_size(prg_rom_size_lsb, prg_rom_size_msb);

            let chr_rom_size_lsb = rom[5] as u16;
            let chr_rom_size_msb = (rom[9] & 0xF0) as u16;

            let chr_rom_size = Ines2::get_chr_rom_size(chr_rom_size_lsb, chr_rom_size_msb);

            if rom[7] & 0b00001100 == 8
                && (prg_rom_size as usize + chr_rom_size as usize) < rom.len()
            {
                return Ok(Box::new(Ines2));
            }

            if rom[7] & 0b00001100 == 4 {
                return Ok(Box::new(ArchaicInes));
            }

            if rom[7] & 0b00001100 == 0 && Self::range_all_zeros(rom, 12, 16) {
                return Ok(Box::new(Ines));
            }

            return Ok(Box::new(Ines07));
        }

        Err(ParseError::UnsupportedFormat)
    }

    /// Parses a ROM file from raw bytes.
    ///
    /// Auto-detects the ROM format (iNES, NES 2.0, archaic iNES, etc.)
    /// from the header and extracts all metadata. The raw data is stored
    /// in [`data`](RomFile::data) and a SHA-256 checksum is computed.
    ///
    /// # Arguments
    ///
    /// * `data` — The complete ROM file as a byte slice.
    /// * `name` — An optional human-readable name (e.g., the file name).
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError`] if:
    /// - The data is too short to contain a valid header ([`ParseError::InvalidHeader`]).
    /// - The ROM format is not recognized ([`ParseError::UnsupportedFormat`]).
    /// - The header declares sizes larger than the file ([`ParseError::SizeBiggerThanFile`]).
    pub fn load(data: &[u8], name: Option<String>) -> Result<RomFile, ParseError> {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash: [u8; 32] = hasher.finalize().into();

        let rom_type = RomFile::get_rom_type(data)?;
        let mut rom_file = rom_type.parse(data, name)?;
        rom_file.data = data.to_vec();
        rom_file.data_checksum = hash;
        Ok(rom_file)
    }

    /// Extracts the PRG ROM region as a read-only [`Memory`] device.
    ///
    /// This is used internally to populate the CPU memory map at addresses
    /// `$8000`-`$FFFF`.
    pub fn get_prg_rom(&self) -> Memory {
        let mut rom = Rom::new(self.prg_memory.prg_rom_size as usize);

        let mut start = 16usize;

        if self.trainer_present {
            start += 512;
        }

        rom.load(
            self.data[start..start + self.prg_memory.prg_rom_size as usize]
                .to_vec()
                .into_boxed_slice(),
        );
        Memory::Rom(rom)
    }

    /// Extracts the CHR ROM region as a read-only [`Memory`] device, if present.
    ///
    /// Returns `None` when the ROM uses CHR RAM instead of CHR ROM
    /// (i.e., `chr_rom_size == 0`).
    pub fn get_chr_rom(&self) -> Option<Memory> {
        if self.chr_memory.chr_rom_size == 0 {
            return None;
        }

        let mut rom = Rom::new(self.chr_memory.chr_rom_size as usize);

        let mut start = 16usize;

        if self.trainer_present {
            start += 512;
        }

        rom.load(
            self.data[start + self.prg_memory.prg_rom_size as usize
                ..start
                    + self.prg_memory.prg_rom_size as usize
                    + self.chr_memory.chr_rom_size as usize]
                .to_vec()
                .into_boxed_slice(),
        );
        Some(Memory::Rom(rom))
    }

    /// Extracts the PRG RAM region as a writable [`Memory`] device.
    ///
    /// This is mapped at CPU addresses `$6000`-`$7FFF` and may be
    /// battery-backed for save data.
    pub fn get_prg_ram(&self) -> Memory {
        let mut ram = Ram::new(self.prg_memory.prg_ram_size as usize);

        let mut start = 16usize;

        if self.trainer_present {
            start += 512;
        }

        ram.load(
            self.data[start..start + self.prg_memory.prg_rom_size as usize]
                .to_vec()
                .into_boxed_slice(),
        );

        Memory::Ram(ram)
    }

    /// Creates the nametable memory for the PPU based on the ROM's mirroring mode.
    ///
    /// Returns a [`Memory`] device configured for either horizontal or vertical
    /// nametable mirroring, as specified by the ROM header.
    pub fn get_nametable_memory(&self) -> Memory {
        let mirroring = match self.hardwired_nametable_layout {
            true => NametableArrangement::Vertical,
            false => NametableArrangement::Horizontal,
        };
        Memory::NametableMemory(NametableMemory::new(mirroring))
    }
}

impl From<&RomFile> for RomFile {
    fn from(rom: &RomFile) -> Self { rom.clone() }
}

impl From<&[u8]> for RomFile {
    fn from(data: &[u8]) -> Self { RomFile::load(data, None).expect("Failed to parse ROM data") }
}

impl From<&(&[u8], String)> for RomFile {
    fn from((data, name): &(&[u8], String)) -> Self {
        RomFile::load(data, Some(name.clone())).expect("Failed to parse ROM data")
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<&String> for RomFile {
    fn from(path: &String) -> Self {
        use std::fs::File;
        use std::io::Read;

        let mut data = Vec::new();
        File::open(path)
            .unwrap_or_else(|e| panic!("Failed to open ROM file '{}': {}", path, e))
            .read_to_end(&mut data)
            .unwrap_or_else(|e| panic!("Failed to read ROM file '{}': {}", path, e));

        RomFile::load(&data, Some(path.clone())).expect("Failed to parse ROM data")
    }
}

/// A builder for constructing [`RomFile`] instances programmatically.
///
/// This is primarily useful for testing when you need a ROM with specific
/// metadata but no actual ROM data.
///
/// # Example
///
/// ```rust
/// use monsoon_core::emulation::rom::RomBuilder;
///
/// let rom = RomBuilder::new()
///     .prg_rom_size(32 * 1024)
///     .chr_rom_size(8 * 1024)
///     .mapper_number(0)
///     .hardwired_nametable_layout(true) // vertical mirroring
///     .build();
///
/// assert_eq!(rom.mapper_number, 0);
/// ```
pub struct RomBuilder {
    name: Option<String>,
    prg_rom_size: u32,
    chr_rom_size: u32,
    mapper_number: u16,
    default_expansion_device: u8,
    misc_rom_count: u8,
    extended_console_type: Option<u8>,
    vs_system_hardware_type: Option<u8>,
    vs_system_ppu_type: Option<u8>,
    cpu_ppu_timing: u8,
    chr_nvram_size: u32,
    chr_ram_size: u32,
    prg_nvram_size: u32,
    prg_ram_size: u32,
    console_type: u8,
    hardwired_nametable_layout: bool,
    is_battery_backed: bool,
    trainer_present: bool,
    alternative_nametables: bool,
    submapper_number: u8,
}

impl Default for RomBuilder {
    fn default() -> Self {
        Self {
            name: None,
            prg_rom_size: 0,
            chr_rom_size: 0,
            mapper_number: 0,
            default_expansion_device: 0,
            misc_rom_count: 0,
            extended_console_type: None,
            vs_system_hardware_type: None,
            vs_system_ppu_type: None,
            cpu_ppu_timing: 0,
            chr_nvram_size: 0,
            chr_ram_size: 0,
            prg_nvram_size: 0,
            prg_ram_size: 8 * 1024,
            console_type: 0,
            hardwired_nametable_layout: false,
            is_battery_backed: false,
            trainer_present: false,
            alternative_nametables: false,
            submapper_number: 0,
        }
    }
}

impl RomBuilder {
    /// Creates a new builder with default values (mapper 0, 8 KB PRG RAM,
    /// all other sizes zero).
    pub fn new() -> Self { Self::default() }

    /// Sets the PRG ROM size in bytes.
    pub fn prg_rom_size(mut self, size: u32) -> Self {
        self.prg_rom_size = size;
        self
    }

    /// Sets the CHR ROM size in bytes.
    pub fn chr_rom_size(mut self, size: u32) -> Self {
        self.chr_rom_size = size;
        self
    }

    /// Sets the iNES mapper number.
    pub fn mapper_number(mut self, number: u16) -> Self {
        self.mapper_number = number;
        self
    }

    /// Sets the default expansion device (NES 2.0).
    pub fn default_expansion_device(mut self, device: u8) -> Self {
        self.default_expansion_device = device;
        self
    }

    /// Sets the miscellaneous ROM count (NES 2.0).
    pub fn misc_rom_count(mut self, count: u8) -> Self {
        self.misc_rom_count = count;
        self
    }

    /// Sets the extended console type (NES 2.0).
    pub fn extended_console_type(mut self, console_type: Option<u8>) -> Self {
        self.extended_console_type = console_type;
        self
    }

    /// Sets the VS System hardware type.
    pub fn vs_system_hardware_type(mut self, hardware_type: Option<u8>) -> Self {
        self.vs_system_hardware_type = hardware_type;
        self
    }

    /// Sets the VS System PPU type.
    pub fn vs_system_ppu_type(mut self, ppu_type: Option<u8>) -> Self {
        self.vs_system_ppu_type = ppu_type;
        self
    }

    /// Sets the CPU/PPU timing mode (0 = NTSC, 1 = PAL, 2 = Multi, 3 = Dendy).
    pub fn cpu_ppu_timing(mut self, timing: u8) -> Self {
        self.cpu_ppu_timing = timing;
        self
    }

    /// Sets the CHR NVRAM (non-volatile) size in bytes.
    pub fn chr_nvram_size(mut self, size: u32) -> Self {
        self.chr_nvram_size = size;
        self
    }

    /// Sets the CHR RAM (volatile) size in bytes.
    pub fn chr_ram_size(mut self, size: u32) -> Self {
        self.chr_ram_size = size;
        self
    }

    /// Sets the PRG NVRAM (non-volatile / battery-backed) size in bytes.
    pub fn prg_nvram_size(mut self, size: u32) -> Self {
        self.prg_nvram_size = size;
        self
    }

    /// Sets the PRG RAM (volatile) size in bytes.
    pub fn prg_ram_size(mut self, size: u32) -> Self {
        self.prg_ram_size = size;
        self
    }

    /// Sets the console type (0 = NES, 1 = VS System, 2 = Playchoice-10, 3 = Extended).
    pub fn console_type(mut self, console_type: u8) -> Self {
        self.console_type = console_type;
        self
    }

    /// Sets the nametable mirroring (`true` = vertical, `false` = horizontal).
    pub fn hardwired_nametable_layout(mut self, value: bool) -> Self {
        self.hardwired_nametable_layout = value;
        self
    }

    /// Sets whether the cartridge has battery-backed memory.
    pub fn battery_backed(mut self, value: bool) -> Self {
        self.is_battery_backed = value;
        self
    }

    /// Sets whether a 512-byte trainer is present in the ROM.
    pub fn trainer_present(mut self, value: bool) -> Self {
        self.trainer_present = value;
        self
    }

    /// Sets whether the ROM uses alternative nametable layouts.
    pub fn alternative_nametables(mut self, value: bool) -> Self {
        self.alternative_nametables = value;
        self
    }

    /// Sets the submapper number (NES 2.0).
    pub fn submapper_number(mut self, number: u8) -> Self {
        self.submapper_number = number;
        self
    }

    /// Sets the ROM name.
    pub fn name(mut self, value: Option<String>) -> Self {
        self.name = value;
        self
    }

    /// Consumes the builder and produces a [`RomFile`].
    ///
    /// The resulting ROM will have an empty `data` field and a zeroed checksum.
    pub fn build(self) -> RomFile {
        RomFile {
            name: self.name,
            prg_memory: PrgMemory::new(self.prg_rom_size, self.prg_ram_size, self.prg_nvram_size),
            chr_memory: ChrMemory::new(self.chr_rom_size, self.chr_ram_size, self.chr_nvram_size),
            mapper_number: self.mapper_number,
            default_expansion_device: self.default_expansion_device,
            misc_rom_count: self.misc_rom_count,
            extended_console_type: self.extended_console_type,
            vs_system_hardware_type: self.vs_system_hardware_type,
            vs_system_ppu_type: self.vs_system_ppu_type,
            cpu_ppu_timing: self.cpu_ppu_timing,
            console_type: self.console_type,
            hardwired_nametable_layout: self.hardwired_nametable_layout,
            is_battery_backed: self.is_battery_backed,
            trainer_present: self.trainer_present,
            alternative_nametables: self.alternative_nametables,
            submapper_number: self.submapper_number,
            data_checksum: [0; 32],
            data: Vec::new(),
        }
    }
}
