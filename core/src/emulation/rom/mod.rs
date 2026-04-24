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

use num_enum::{FromPrimitive, IntoPrimitive};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::emulation::mapper::nametable_mapping::NametableArrangement;
use crate::emulation::mem::{MemoryDevice, Ram, Rom};
use crate::emulation::rom::formats::archaic_ines::ArchaicInes;
use crate::emulation::rom::formats::ines::Ines;
use crate::emulation::rom::formats::ines2::Ines2;
use crate::emulation::rom::formats::ines_07::Ines07;

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
/// println!("Mapper: {}", rom.mapper);
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
    pub mapper: RomMapper,
    /// Default expansion device identifier (NES 2.0).
    pub default_expansion_device: ExpansionDevice,
    /// Number of miscellaneous ROM areas (NES 2.0).
    pub misc_rom_count: u8,
    /// Extended console type (NES 2.0), if applicable.
    pub extended_console_type: Option<ExtendedConsoleType>,
    /// VS System hardware type, if applicable.
    pub vs_system_hardware_type: Option<VsHardwareType>,
    /// VS System PPU type, if applicable.
    pub vs_system_ppu_type: Option<VsSystemPpuType>,
    /// CPU/PPU timing mode (0 = NTSC, 1 = PAL, 2 = Multi-region, 3 = Dendy).
    pub timing_region: RomTimingRegion,
    /// Console type (0 = NES/Famicom, 1 = VS System, 2 = Playchoice-10, 3 =
    /// Extended).
    pub console_type: ConsoleType,
    /// Nametable mirroring mode from header bit 0 (`true` = vertical, `false` =
    /// horizontal).
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
    /// Raw ROM file bytes. Skipped during serialization to reduce save state
    /// size.
    #[serde(skip)]
    pub data: Vec<u8>,
}

#[derive(
    Debug,
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    FromPrimitive,
    IntoPrimitive,
    Serialize,
    Deserialize,
)]
#[repr(u8)]
pub enum ExpansionDevice {
    Unspecified = 0,
    StandardController = 1,
    FourScore = 2,
    FourScoreSimple = 3,
    VsSystem1P4016 = 4,
    VsSystem1P4017 = 5,
    VsZapper = 7,
    Zapper4017 = 8,
    TwoZappers = 9,
    BandaiHyperShotLightgun = 10,
    PowerPadSideA = 11,
    PowerPadSideB = 12,
    FamilyTrainerSideA = 13,
    FamilyTrainerSideB = 14,
    ArkanoidVausNes = 15,
    ArkanoidVausFamicom = 16,
    TwoVausPlusDataRecorder = 17,
    KonamiHyperShot = 18,
    CoconutsPachinko = 19,
    ExcitingBoxingPunchingBag = 20,
    JissenMahjong = 21,
    PartyTap = 22,
    OekaKidsTablet = 23,
    SunsoftBarcodeBattler = 24,
    MiraclePianoKeyboard = 25,
    PokkunMoguraaTapTapMat = 26,
    TopRider = 27,
    DoubleFisted = 28,
    Famicom3dSystem = 29,
    DoremikkoKeyboard = 30,
    RobGyromite = 31,
    FamicomDataRecorder = 32,
    AsciiTurboFile = 33,
    IgsStorageBattleBox = 34,
    FamilyBasicKeyboardPlusDataRecorder = 35,
    PecKeyboard = 36,
    Bit79Keyboard = 37,
    SuborKeyboard = 38,
    SuborKeyboardPlusMacroWinnersMouse = 39,
    SuborKeyboardPlusSuborMouse4016 = 40,
    SnesMouse4016 = 41,
    Multicart = 42,
    TwoSnesControllers = 43,
    RacerMateBicycle = 44,
    UForce = 45,
    RobStackUp = 46,
    CityPatrolmanLightgun = 47,
    SharpC1CassetteInterface = 48,
    StandardControllerSwappedLayout = 49,
    ExcaliburSudokuPad = 50,
    AblPinball = 51,
    GoldenNuggetCasinoExtraButtons = 52,
    KedaKeyboard = 53,
    SuborKeyboardPlusSuborMouse4017 = 54,
    PortTestController = 55,
    BandaiMultiGamePlayerGamepadButtons = 56,
    VenomTvDanceMat = 57,
    LgTvRemoteControl = 58,
    FamicomNetworkController = 59,
    KingFishingController = 60,
    CroakyKaraokeController = 61,
    KingwonKeyboard = 62,
    ZechengKeyboard = 63,
    SuborKeyboardPlusL90RotatedPs2Mouse4017 = 64,
    Ps2KeyboardUM6578PlusPs2Mouse4017 = 65,
    Ps2MouseUM6578 = 66,
    YuxingMouse4016 = 67,
    SuborKeyboardPlusYuxingMouse4016 = 68,
    GiggleTvPump = 69,
    BBKKeyboardPlusR90RotatedPs2Mouse4017 = 70,
    MagicalCooking = 71,
    SnesMouse4017 = 72,
    Zapper4016 = 73,
    ArkanoidVausControllerPrototype = 74,
    TvMahjongGameController = 75,
    MahjongGekitouDensetsuController = 76,
    SuborKeyboardPlusXInvertedPs2Mouse4017 = 77,
    IbmPcXtKeyboard = 78,
    SuborKeyboardPlusMegaBookMouse = 79,
    #[num_enum(catch_all)]
    Unknown(u8),
}

impl Display for ExpansionDevice {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str: &str = match self {
            ExpansionDevice::Unspecified => "Unspecified",
            ExpansionDevice::StandardController => "Standard NES/Famicom Controllers",
            ExpansionDevice::FourScore => {
                "NES Four Score/Satellite w/ two additional standard controllers"
            }
            ExpansionDevice::FourScoreSimple => {
                "Famicom Four Player Adapter w/ two additional standard controllers using the \
                 \"simple\" protocol"
            }
            ExpansionDevice::VsSystem1P4016 => "Vs. System (One Player via Port 1)",
            ExpansionDevice::VsSystem1P4017 => "Vs. System (One Player via Port 2)",
            ExpansionDevice::VsZapper => "Vs. Zapper",
            ExpansionDevice::Zapper4017 => "Zapper (via Port 2)",
            ExpansionDevice::TwoZappers => "Two Zappers",
            ExpansionDevice::BandaiHyperShotLightgun => "Bandai Hyper Shot Lightgun",
            ExpansionDevice::PowerPadSideA => "Power Pad Side A",
            ExpansionDevice::PowerPadSideB => "Power Pad Side B",
            ExpansionDevice::FamilyTrainerSideA => "Family Trainer Side A",
            ExpansionDevice::FamilyTrainerSideB => "Family Trainer Side B",
            ExpansionDevice::ArkanoidVausNes => "Arkanoid Vaus Controller (NES)",
            ExpansionDevice::ArkanoidVausFamicom => "Arkanoid Vaus Controller (Famicom)",
            ExpansionDevice::TwoVausPlusDataRecorder => {
                "Two Arkanoid Vaus Controllers + Famicom Data Recorder"
            }
            ExpansionDevice::KonamiHyperShot => "Konami Hyper Shot Controller",
            ExpansionDevice::CoconutsPachinko => "Coconuts Pachinko Controller",
            ExpansionDevice::ExcitingBoxingPunchingBag => "Exciting Boxing Punching Bag",
            ExpansionDevice::JissenMahjong => "Jissen Mahjong Controller",
            ExpansionDevice::PartyTap => "米澤 (Yonezawa) Party Tap",
            ExpansionDevice::OekaKidsTablet => "Oeka Kids Tablet",
            ExpansionDevice::SunsoftBarcodeBattler => "Sunsoft Barcode Battler",
            ExpansionDevice::MiraclePianoKeyboard => "Miracle Piano Keyboard",
            ExpansionDevice::PokkunMoguraaTapTapMat => "Pokkun Moguraa Tap-tap Mat1",
            ExpansionDevice::TopRider => "Top Rider",
            ExpansionDevice::DoubleFisted => "Double Fisted",
            ExpansionDevice::Famicom3dSystem => "Famicom 3D System",
            ExpansionDevice::DoremikkoKeyboard => "Doremikko Keyboard",
            ExpansionDevice::RobGyromite => "R.O.B Gyromite",
            ExpansionDevice::FamicomDataRecorder => "Famicom Data Recorder",
            ExpansionDevice::AsciiTurboFile => "ASCII Turbo File",
            ExpansionDevice::IgsStorageBattleBox => "IGS Storage Battle Box",
            ExpansionDevice::FamilyBasicKeyboardPlusDataRecorder => {
                "Family Basic Keyboard + Famicom Data Recorder"
            }
            ExpansionDevice::PecKeyboard => "东达 (Dōngdá) PEC Keyboard",
            ExpansionDevice::Bit79Keyboard => "普澤 (Pǔzé, a.k.a. Bit Corp.) Bit-79 Keyboard",
            ExpansionDevice::SuborKeyboard => "小霸王 (Xiǎobàwáng, a.k.a. Subor) Keyboard",
            ExpansionDevice::SuborKeyboardPlusMacroWinnersMouse => {
                "小霸王 (Xiǎobàwáng, a.k.a. Subor) Keyboard + Macro Winners Mouse"
            }
            ExpansionDevice::SuborKeyboardPlusSuborMouse4016 => {
                "小霸王 (Xiǎobàwáng, a.k.a. Subor) Keyboard + Subor Mouse (via Port 1)"
            }
            ExpansionDevice::SnesMouse4016 => "SNES Mouse (via Port 1)",
            ExpansionDevice::Multicart => "Multicart",
            ExpansionDevice::TwoSnesControllers => "Two SNES Controllers",
            ExpansionDevice::RacerMateBicycle => "RacerMate Bicycle",
            ExpansionDevice::UForce => "U-Force",
            ExpansionDevice::RobStackUp => "R.O.B Stack-Up",
            ExpansionDevice::CityPatrolmanLightgun => "City Patrolman Lightgun",
            ExpansionDevice::SharpC1CassetteInterface => "Sharp C1 Cassette Interface",
            ExpansionDevice::StandardControllerSwappedLayout => {
                "Standard Controller with swapped Left-Right/Up-Down/B-A"
            }
            ExpansionDevice::ExcaliburSudokuPad => "Excalibur Sudoku Pad",
            ExpansionDevice::AblPinball => "ABL Pinball",
            ExpansionDevice::GoldenNuggetCasinoExtraButtons => "Golden Nugget Casino Controller",
            ExpansionDevice::KedaKeyboard => "科达 (Kēdá) Keyboard",
            ExpansionDevice::SuborKeyboardPlusSuborMouse4017 => {
                "小霸王 (Xiǎobàwáng, a.k.a. Subor) Keyboard + Subor Mouse (via Port 2)"
            }
            ExpansionDevice::PortTestController => "Port test controller",
            ExpansionDevice::BandaiMultiGamePlayerGamepadButtons => {
                "Bandai Multi Game Player Gamepad"
            }
            ExpansionDevice::VenomTvDanceMat => "Venom TV Dance Mat",
            ExpansionDevice::LgTvRemoteControl => "LG TV Remote Control",
            ExpansionDevice::FamicomNetworkController => "Famicom Network Controller",
            ExpansionDevice::KingFishingController => "King Fishing Controller",
            ExpansionDevice::CroakyKaraokeController => "Croaky Karaoke Controller",
            ExpansionDevice::KingwonKeyboard => "科王 (Kēwáng, a.k.a. Kingwon) Keyboard",
            ExpansionDevice::ZechengKeyboard => "泽诚 (Zéchéng) Keyboard",
            ExpansionDevice::SuborKeyboardPlusL90RotatedPs2Mouse4017 => {
                "小霸王 (Xiǎobàwáng, a.k.a. Subor) Keyboard + PS/2 mouse rotated left (via Port 2)"
            }
            ExpansionDevice::Ps2KeyboardUM6578PlusPs2Mouse4017 => {
                "PS/2 Keyboard in UM6578 PS/2 port + PS/2 Mouse (via Port 2)"
            }
            ExpansionDevice::Ps2MouseUM6578 => "PS/2 Mouse in UM6578 PS/2 port",
            ExpansionDevice::YuxingMouse4016 => "裕兴 (Yùxìng) Mouse (via Port 1)",
            ExpansionDevice::SuborKeyboardPlusYuxingMouse4016 => {
                "小霸王 (Xiǎobàwáng, a.k.a. Subor )Keyboard + 裕兴 (Yùxìng) Mouse (via Port 1)"
            }
            ExpansionDevice::GiggleTvPump => "Giggle TV Pump",
            ExpansionDevice::BBKKeyboardPlusR90RotatedPs2Mouse4017 => {
                "步步高 (Bùbùgāo, a.k.a. BBK) Keyboard + PS/2 mouse rotated right (via Port 2)"
            }
            ExpansionDevice::MagicalCooking => "Magical Cooking",
            ExpansionDevice::SnesMouse4017 => "SNES Mouse (via Port 2)",
            ExpansionDevice::Zapper4016 => "Zapper (via Port 1)",
            ExpansionDevice::ArkanoidVausControllerPrototype => {
                "Arkanoid Vaus Controller (Prototype)"
            }
            ExpansionDevice::TvMahjongGameController => "TV 麻雀 Game (TV Mahjong Game) Controller",
            ExpansionDevice::MahjongGekitouDensetsuController => {
                "麻雀激闘伝説 (Mahjong Gekitou Densetsu) Controller"
            }
            ExpansionDevice::SuborKeyboardPlusXInvertedPs2Mouse4017 => {
                "小霸王 (Xiǎobàwáng, a.k.a. Subor) Keyboard + X-inverted PS/2 mouse via (Port 2)"
            }
            ExpansionDevice::IbmPcXtKeyboard => "IBM PC/XT Keyboard",
            ExpansionDevice::SuborKeyboardPlusMegaBookMouse => {
                "小霸王 (Xiǎobàwáng, a.k.a. Subor) Keyboard + Mega Book Mouse"
            }
            ExpansionDevice::Unknown(_) => "Unknown Expansion Device",
        };

        let id: u8 = (*self).into();

        write!(f, "{str} (Header: {})", id)
    }
}

#[derive(
    Debug,
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    FromPrimitive,
    IntoPrimitive,
    Serialize,
    Deserialize,
)]
#[repr(u8)]
pub enum ExtendedConsoleType {
    NesFamicom = 0,
    VsSystem = 1,
    Playchoice10 = 2,
    DecimalModeFamiclone = 3,
    EPSMFamicom = 4,
    VT01 = 5,
    VT02 = 6,
    VT03 = 7,
    VT09 = 8,
    VT32 = 9,
    VT369 = 0xA,
    UM6578 = 0xB,
    FamicomNetworkSystem = 0xC,
    #[num_enum(catch_all)]
    Unknown(u8),
}

impl Display for ExtendedConsoleType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str: &str = match self {
            ExtendedConsoleType::NesFamicom => "Nes/Famicom/Dendy",
            ExtendedConsoleType::VsSystem => "Nintendo Vs. System",
            ExtendedConsoleType::Playchoice10 => "Nintendo Playchoice 10",
            ExtendedConsoleType::DecimalModeFamiclone => "Famiclone w/ Decimal Mode CPU",
            ExtendedConsoleType::EPSMFamicom => {
                "Nes/Famicom/Dendy w/ EPSM module or plug-through Cartridge"
            }
            ExtendedConsoleType::VT01 => "V.R. Technology VT01 with red/cyan STN palette",
            ExtendedConsoleType::VT02 => "V.R Technology VT02",
            ExtendedConsoleType::VT03 => "V.R Technology VT03",
            ExtendedConsoleType::VT09 => "V.R Technology VT09",
            ExtendedConsoleType::VT32 => "V.R Technology VT32",
            ExtendedConsoleType::VT369 => "V.R Technology VT369",
            ExtendedConsoleType::UM6578 => "UMC UM6578",
            ExtendedConsoleType::FamicomNetworkSystem => "Famicom Network System",
            ExtendedConsoleType::Unknown(_) => "Unknown Extended Console Type",
        };

        let id: u8 = (*self).into();

        write!(f, "{str} (Header: {})", id)
    }
}

#[derive(
    Debug,
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    FromPrimitive,
    IntoPrimitive,
    Serialize,
    Deserialize,
)]
#[repr(u8)]
pub enum VsHardwareType {
    UniSystem = 0,
    UnisystemRbiBaseball = 1,
    UnisystemTkoBoxing = 2,
    UnisystemSuperXevious = 3,
    UnisystemVcIceClimberJapan = 4,
    DualSystem = 5,
    DualSystemRaidOnBungelingBay = 6,
    #[num_enum(catch_all)]
    Unknown(u8),
}

impl Display for VsHardwareType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str: &str = match self {
            VsHardwareType::UniSystem => "Vs. Unisystem (normal)",
            VsHardwareType::UnisystemRbiBaseball => "Vs. Unisystem (RBI Baseball protection)",
            VsHardwareType::UnisystemTkoBoxing => "Vs. Unisystem (TKO Boxing protection)",
            VsHardwareType::UnisystemSuperXevious => "Vs. Unisystem (Super Xevious protection)",
            VsHardwareType::UnisystemVcIceClimberJapan => {
                "Vs. Unisystem (Vs. Ice Climber Japan protection)"
            }
            VsHardwareType::DualSystem => "Vs. Dual System (normal)",
            VsHardwareType::DualSystemRaidOnBungelingBay => {
                "Vs. Dual System (Raid on Bungeling Bay protection)"
            }
            VsHardwareType::Unknown(_) => "Unknown Vs. System hardware type",
        };

        let id: u8 = (*self).into();

        write!(f, "{str} (Header: {})", id)
    }
}

#[derive(
    Debug,
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    FromPrimitive,
    IntoPrimitive,
    Serialize,
    Deserialize,
)]
#[repr(u8)]
pub enum VsSystemPpuType {
    RP2C03 = 0,
    RP2C04_0001 = 2,
    RP2C04_0002 = 3,
    RP2C04_0003 = 4,
    RP2C04_0004 = 5,
    RC2C05_01 = 8,
    RC2C05_02 = 9,
    RC2C05_03 = 0xA,
    RC2C05_04 = 0xB,
    #[num_enum(catch_all)]
    Unknown(u8),
}

impl Display for VsSystemPpuType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str: &str = match self {
            VsSystemPpuType::RP2C03 => "Any RP2C03/RC2C03 Variant",
            VsSystemPpuType::RP2C04_0001 => "RP2C04-0001",
            VsSystemPpuType::RP2C04_0002 => "RP2C04-0002",
            VsSystemPpuType::RP2C04_0003 => "RP2C04-0003",
            VsSystemPpuType::RP2C04_0004 => "RP2C04-0004",
            VsSystemPpuType::RC2C05_01 => "RC2C05-01",
            VsSystemPpuType::RC2C05_02 => "RC2C05-02",
            VsSystemPpuType::RC2C05_03 => "RC2C05-03",
            VsSystemPpuType::RC2C05_04 => "RC2C05-04",
            VsSystemPpuType::Unknown(_) => "Unknown Vs. System PPU",
        };

        let id: u8 = (*self).into();

        write!(f, "{str} (Header: {})", id)
    }
}

#[derive(
    Debug,
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    FromPrimitive,
    IntoPrimitive,
    Serialize,
    Deserialize,
)]
#[repr(u8)]
pub enum ConsoleType {
    NesFamicom = 0,
    VsSystem = 1,
    Playchoice10 = 2,
    Extended = 3,
    #[num_enum(catch_all)]
    Unknown(u8),
}

impl Display for ConsoleType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str: &str = match self {
            ConsoleType::NesFamicom => "Nes/Famicom/Dendy",
            ConsoleType::VsSystem => "Nintendo Vs. System",
            ConsoleType::Playchoice10 => "Nintendo Playchoice 10",
            ConsoleType::Extended => "Extended Console Type",
            ConsoleType::Unknown(_) => "Unknown Console Type",
        };

        let id: u8 = (*self).into();

        write!(f, "{str} (Header: {})", id)
    }
}

#[derive(
    Debug,
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    FromPrimitive,
    IntoPrimitive,
    Serialize,
    Deserialize,
)]
#[repr(u8)]
pub enum RomTimingRegion {
    RP2C02 = 0,
    RP2C07 = 1,
    Multi = 2,
    UA6538 = 3,
    #[num_enum(catch_all)]
    Unknown(u8),
}

impl Display for RomTimingRegion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str: &str = match self {
            RomTimingRegion::RP2C02 => "NTSC/RP2C02",
            RomTimingRegion::RP2C07 => "Licensed PAL/RP2C07",
            RomTimingRegion::Multi => "Multiple Regions",
            RomTimingRegion::UA6538 => "Dendy/UA6538",
            RomTimingRegion::Unknown(_) => "Unknown Region",
        };

        let id: u8 = (*self).into();

        write!(f, "{str} (Header: {})", id)
    }
}

#[derive(
    Debug,
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    FromPrimitive,
    IntoPrimitive,
    Serialize,
    Deserialize,
)]
#[repr(u16)]
pub enum RomMapper {
    NRom = 0,
    #[num_enum(catch_all)]
    Unknown(u16),
}

impl Display for RomMapper {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str: &str = match self {
            RomMapper::NRom => "NROM",
            RomMapper::Unknown(_) => "Unknown Mapper",
        };

        let mapper_num: u16 = (*self).into();

        write!(f, "{str} (INes Mapper {})", mapper_num)
    }
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
    /// - The data is too short to contain a valid header
    ///   ([`ParseError::InvalidHeader`]).
    /// - The ROM format is not recognized ([`ParseError::UnsupportedFormat`]).
    /// - The header declares sizes larger than the file
    ///   ([`ParseError::SizeBiggerThanFile`]).
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
    #[doc(hidden)]
    pub fn get_prg_rom(&self) -> Rom {
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
        rom
    }

    /// Extracts the CHR ROM region as a read-only [`Memory`] device, if
    /// present.
    ///
    /// Returns `None` when the ROM uses CHR RAM instead of CHR ROM
    /// (i.e., `chr_rom_size == 0`).
    #[doc(hidden)]
    pub fn get_chr_rom(&self) -> Option<Rom> {
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
        Some(rom)
    }

    /// Extracts the PRG RAM region as a writable [`Memory`] device.
    ///
    /// This is mapped at CPU addresses `$6000`-`$7FFF` and may be
    /// battery-backed for save data.
    #[doc(hidden)]
    pub fn get_prg_ram(&self) -> Option<Ram> {
        if self.prg_memory.prg_ram_size > 0 {
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

            Some(ram)
        } else {
            None
        }
    }

    /// Creates the nametable memory for the PPU based on the ROM's mirroring
    /// mode.
    ///
    /// Returns a [`Memory`] device configured for either horizontal or vertical
    /// nametable mirroring, as specified by the ROM header.
    #[doc(hidden)]
    pub fn get_nametable_memory(&self) -> NametableArrangement {
        let mirroring = match self.hardwired_nametable_layout {
            true => NametableArrangement::Vertical,
            false => NametableArrangement::Horizontal,
        };
        mirroring
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
/// use monsoon_core::emulation::rom::{RomBuilder, RomMapper};
///
/// let rom = RomBuilder::new()
///     .prg_rom_size(32 * 1024)
///     .chr_rom_size(8 * 1024)
///     .mapper_number(0)
///     .hardwired_nametable_layout(true) // vertical mirroring
///     .build();
///
/// assert_eq!(rom.mapper, RomMapper::NRom);
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
    rom_timing_region: u8,
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
            rom_timing_region: 0,
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
        self.rom_timing_region = timing;
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

    /// Sets the console type (0 = NES, 1 = VS System, 2 = Playchoice-10, 3 =
    /// Extended).
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
            mapper: RomMapper::from(self.mapper_number),
            default_expansion_device: ExpansionDevice::from(self.default_expansion_device),
            misc_rom_count: self.misc_rom_count,
            extended_console_type: self
                .extended_console_type
                .map(|t| ExtendedConsoleType::from(t)),
            vs_system_hardware_type: self
                .vs_system_hardware_type
                .map(|t| VsHardwareType::from(t)),
            vs_system_ppu_type: self.vs_system_ppu_type.map(|t| VsSystemPpuType::from(t)),
            timing_region: RomTimingRegion::from(self.rom_timing_region),
            console_type: ConsoleType::from(self.console_type),
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
