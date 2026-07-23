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
use strum::EnumIter;

use crate::emulation::mapper::nametable_mapping::NametableArrangement;
use crate::emulation::mem::Memory;
use crate::emulation::nes::Nes;
use crate::emulation::rom::formats::archaic_ines::ArchaicInes;
use crate::emulation::rom::formats::ines::Ines;
use crate::emulation::rom::formats::ines2::Ines2;
use crate::emulation::rom::formats::ines_07::Ines07;

/// Errors that can occur while parsing a ROM file.
#[derive(Debug)]
pub enum ParseError {
    /// The sizes declared in the ROM header exceed the actual file length.
    SizeBiggerThanFile,
    /// The ROM data is too short to contain a valid header (minimum 16 bytes).
    InvalidHeader,
    /// The ROM format is not recognized (missing `NES\x1A` magic bytes).
    UnsupportedFormat,
    IoError(std::io::Error),
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
            ParseError::IoError(err) => {
                write!(f, "IO Error while parsing rom")?;
                write!(f, "{}", err)
            }
        }
    }
}

impl Error for ParseError {}

impl From<std::io::Error> for ParseError {
    fn from(err: std::io::Error) -> Self { ParseError::IoError(err) }
}

/// Trait for ROM format parsers.
///
/// Each supported ROM format (iNES, NES 2.0, etc.) implements this trait.
/// Users should not need to call parsers directly; use [`RomFile::load()`]
/// instead, which auto-detects the format.
#[doc(hidden)]
pub trait RomParser: Debug {
    fn get_name(&self) -> &str;
    fn parse(&self, rom: &[u8], name: Option<&String>) -> Result<RomFile, ParseError>;
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
/// # let mut raw_bytes: &[u8] = &[];
/// let rom = RomFile::load(&mut raw_bytes, Some(&"my_game.nes".to_string()), false).expect(
///     "invalid
/// ROM",
/// );
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
    pub checksum_headerless: [u8; 32],
    /// Raw ROM file bytes. Skipped during serialization to reduce save state
    /// size.
    #[serde(skip)]
    pub data: Vec<u8>,
    pub original_name: Option<String>,
    pub format_name: String,
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
    EnumIter,
    FromPrimitive,
    IntoPrimitive,
    Serialize,
    Deserialize,
)]
#[repr(u8)]
#[serde(into = "u8", from = "u8")]
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
    EnumIter,
    FromPrimitive,
    IntoPrimitive,
    Serialize,
    Deserialize,
)]
#[repr(u8)]
#[serde(into = "u8", from = "u8")]
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
            ExtendedConsoleType::DecimalModeFamiclone => "Famiclone with Decimal Mode CPU",
            ExtendedConsoleType::EPSMFamicom => {
                "Nes/Famicom/Dendy with EPSM module or plug-through Cartridge"
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
    EnumIter,
    FromPrimitive,
    IntoPrimitive,
    Serialize,
    Deserialize,
)]
#[repr(u8)]
#[serde(into = "u8", from = "u8")]
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
    EnumIter,
    FromPrimitive,
    IntoPrimitive,
    Serialize,
    Deserialize,
)]
#[repr(u8)]
#[serde(into = "u8", from = "u8")]
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
    EnumIter,
    FromPrimitive,
    IntoPrimitive,
    Serialize,
    Deserialize,
)]
#[repr(u8)]
#[serde(into = "u8", from = "u8")]
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
    EnumIter,
    FromPrimitive,
    IntoPrimitive,
    Serialize,
    Deserialize,
)]
#[repr(u8)]
#[serde(into = "u8", from = "u8")]
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
    EnumIter,
    FromPrimitive,
    IntoPrimitive,
    Serialize,
    Deserialize,
)]
#[repr(u16)]
#[serde(into = "u16", from = "u16")]
pub enum RomMapper {
    NRom = 0,
    MMC1 = 1,
    UxROM = 2,
    CNROM = 3,
    MMC3 = 4,
    MMC5 = 5,
    FrontFareastMagicCard1and2MB = 6,
    AxROM = 7,
    FrontFareastMagicCard1and2MBSub4 = 8,
    MMC2 = 9,
    MMC4 = 10,
    ColorDreams = 11,
    SL5020BandFrontFareastMagicCard4MB = 12,
    CPROM = 13,
    SL1632 = 14,
    K1029andK1030P = 15,
    FGC1and2orLZ93D50 = 16,
    FrontFareastSuperMagicCard = 17,
    SS88006 = 18,
    Namco129and163 = 19,
    FamicomDiskSystem = 20,
    VRC4AandC = 21,
    VRC2A = 22,
    VRC2BandVRC4FandE = 23,
    VRC6A = 24,
    VRC2CandVRC4BandD = 25,
    VRC6B = 26,
    VRC4unl = 27,
    Action53 = 28,
    CUFROM = 29,
    UNROM512 = 30,
    NSFSubset = 31,
    G101 = 32,
    TC0190 = 33,
    NINAorBNROM = 34,
    JingtaiASIC8kBWRAM = 35,
    TXC0122000400 = 36,
    SmbTetrisNWCMulticart = 37,
    BitCorpCrimeBusters = 38,
    OversizeBNROM = 39,
    NTDEC2722and2752 = 40,
    Caltron6in1 = 41,
    HackedFDS = 42,
    TONYI = 43,
    MMC3Multicart = 44,
    MulticartGA23C = 45,
    RumbleStation = 46,
    MMC3Multicart2 = 47,
    TC0690 = 48,
    MMC3Multicart3 = 49,
    SMB2N32 = 50,
    BallGames11in1 = 51,
    Realtec8213 = 52,
    Supervision16in1 = 53,
    NovelDiamond201Alias = 54,
    BTLMario1 = 55,
    UnlSMB3 = 56,
    MulticartMapper4 = 57,
    NROMMulticart = 58,
    BMCT3H53 = 59,
    NROM128ResetMulticart = 60,
    MulticartMapper5 = 61,
    MulticartMapper6 = 62,
    NTDECMulticartTH29913 = 63,
    RAMBO1 = 64,
    H3001 = 65,
    GxROM = 66,
    Sunsoft3 = 67,
    Sunsoft4 = 68,
    SunsoftFME7 = 69,
    BandaiMapperFamilyMat = 70,
    Codemasters = 71,
    JF17 = 72,
    VRC3 = 73,
    Waixing43393 = 74,
    VRC1 = 75,
    Namcot108FamCHR128Coarse = 76,
    NapoleonSenki = 77,
    HolyDiver = 78,
    NINA03and06 = 79,
    X1005 = 80,
    NTDECSuperGun = 81,
    X1017 = 82,
    Cony = 83,
    PCSMB2J = 84,
    VRC7 = 85,
    JF13 = 86,
    J87 = 87,
    Namcot108FamCHR128PPU = 88,
    Sunsoft2B3 = 89,
    JingtaiASICInhibited = 90,
    JY830623C = 91,
    JF17Alt = 92,
    Sunsoft2B3R = 93,
    UN1ROM = 94,
    Namcot108FamVariableArrangement = 95,
    BandaiOekaKids = 96,
    TAMS1 = 97,
    GBCPort = 98,
    VsSystemCNROM = 99,
    NesticleMMC3Hack = 100,
    J87Reverse = 101,
    QuietustDripAlt = 102,
    DokiDokiPanicPirate = 103,
    Pegasus5in1 = 104,
    NesEvent = 105,
    SMB3Bootleg = 106,
    MagicDragon = 107,
    FDSCartridgeMapper = 108,
    GreatWall = 109,
    HoneyPeach = 110,
    GTROMorMMC1Unserial = 111,
    NTDECScrambled = 112,
    NTD8 = 113,
    MMC3CloneScrambled = 114,
    KashengMMC3 = 115,
    SOMARIP = 116,
    FutureMediaMapper = 117,
    TKLSROM = 118,
    TQROM = 119,
    LH15 = 120,
    A9711andA9713 = 121,
    JY043 = 122,
    H2288 = 123,
    SuperGameMegaTypeIIIMapper = 124,
    LH32 = 125,
    TEC9719orING003CorING022RepurposedLines = 126,
    DoubleDragonIIPirate = 127,
    T262 = 128,
    DuplicateNROMMulticart = 129,
    Alternative331 = 130,
    Alternative205 = 131,
    TXC22Fam = 132,
    SachenJovialMasterChu = 133,
    T4A54A = 134,
    Sachen8259AOld = 135,
    Sachen3011 = 136,
    Sachen8259D = 137,
    Sachen8259B = 138,
    Sachen8259C = 139,
    JF11orJF14 = 140,
    Sachen8259A = 141,
    KS7032 = 142,
    NROMCopyProt = 143,
    DeathRaceMapper = 144,
    SA72007 = 145,
    Sachen3015 = 146,
    Sachen3018 = 147,
    SA008A = 148,
    SA0036 = 149,
    SA015 = 150,
    VsSystemVRC1 = 151,
    BandaiFamilyMatPlus = 152,
    FCGWithLZ93D508KiBWRAM = 153,
    NAMCOT3453 = 154,
    MMC1A = 155,
    DIS23C01 = 156,
    FCGDetachedJointRom = 157,
    Tengen800037 = 158,
    FCGWithLZ93D50EEPROM = 159,
    JingtaiASICDuplicate = 160,
    HanjukuHeroMMC1 = 161,
    FS304 = 162,
    FC001 = 163,
    CY20003 = 164,
    MMC2MMC3Mashup = 165,
    IncorrectSuborLearning = 166,
    SuborLearning = 167,
    Racermate = 168,
    Yuxing = 169,
    ShikoGameSyu = 170,
    Floppy1 = 171,
    P4070 = 172,
    IdeaTek = 173,
    NROMMulticartShuffled = 174,
    Kaiser15in1 = 175,
    UNROMCrazyClimber = 180,
    CNROM8KB = 185,
    MMC3Multicart7 = 205,
    Namcot108Fam = 206,
    NS037in0 = 331,
    #[num_enum(catch_all)]
    Unknown(u16),
}

impl Display for RomMapper {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            RomMapper::NRom => "NROM",
            RomMapper::MMC1 => "MMC1",
            RomMapper::MMC1A => "MMC1A",
            RomMapper::NesEvent => "NES-EVENT",
            RomMapper::UxROM => "UxROM",
            RomMapper::UN1ROM => "UN1ROM",
            RomMapper::UNROMCrazyClimber => "UNROM (Crazy Climber)",
            RomMapper::CNROM => "CNROM",
            RomMapper::CNROM8KB => "CNROM (with 8KiB CHR-ROM)",
            RomMapper::MMC3 => "MMC3",
            RomMapper::TKLSROM => "TKSROM/TLSROM",
            RomMapper::TQROM => "TQROM",
            RomMapper::Unknown(_) => "Unknown Mapper",
            RomMapper::MMC5 => "MMC5",
            RomMapper::FrontFareastMagicCard1and2MB => "Front Fareast Magic Card 1/2MB",
            RomMapper::AxROM => "AxROM",
            RomMapper::FrontFareastMagicCard1and2MBSub4 => {
                "Front Fareast Magic Card 1/2MB (using Submapper 4)"
            }
            RomMapper::MMC2 => "MMC2",
            RomMapper::MMC4 => "MMC4",
            RomMapper::ColorDreams => "Color Dreams",
            RomMapper::SL5020BandFrontFareastMagicCard4MB => {
                "哥德 (Gouder) SL-5020B (Sub. 0) or Front Fareast Magic Card 4MB (Sub. 1)"
            }
            RomMapper::CPROM => "CPROM",
            RomMapper::SL1632 => "哥德 (Gouder) SL-1632",
            RomMapper::K1029andK1030P => "Kaiser K-1029 or K-1030P",
            RomMapper::FGC1and2orLZ93D50 => "Bandai FCG-1/2 (Sub. 4) or LZ93D50 (Sub. 5)",
            RomMapper::FrontFareastSuperMagicCard => "Front Fareast Super Magic Card",
            RomMapper::SS88006 => "Jaleco SS 88006",
            RomMapper::Namco129and163 => "Namco 129/163",
            RomMapper::FamicomDiskSystem => "Famicom Disk System",
            RomMapper::VRC4AandC => "Konami VRC4a (Sub. 1) or VRC4c (Sub. 2)",
            RomMapper::VRC2A => "Konami VRC2a",
            RomMapper::VRC2BandVRC4FandE => {
                "Konami VRC2b (Sub. 3), VRC4e (Sub. 2), or VRC4f (Sub. 1)"
            }
            RomMapper::VRC6A => "Konami VRC6a",
            RomMapper::VRC2CandVRC4BandD => {
                "Konami VRC2c (Sub. 3), VRC4b (Sub. 1), or VRC4d (Sub. 2)"
            }
            RomMapper::VRC6B => "Konami VRC6b",
            RomMapper::VRC4unl => "Konami VRC4 unlicensed",
            RomMapper::Action53 => "Action 53",
            RomMapper::CUFROM => "RET-CUFROM",
            RomMapper::UNROM512 => "UNROM 512",
            RomMapper::NSFSubset => "NSF Subset",
            RomMapper::G101 => "Irem G-101",
            RomMapper::TC0190 => "Taito TC0190",
            RomMapper::NINAorBNROM => "NINA-001/002 (Sub. 1) or BNROM (Sub 2.)",
            RomMapper::JingtaiASIC8kBWRAM => "晶太 (Jīngtài) ASIC (with 8KiB WRAM)",
            RomMapper::TXC0122000400 => "TXC 01-22000-400",
            RomMapper::SmbTetrisNWCMulticart => {
                "Nintendo of Europe Multicart Mapper \"Super Mario Bros. + Tetris + Nintendo World \
                 Cup 3-in-1\""
            }
            RomMapper::BitCorpCrimeBusters => "Bit Corp. Crime Busters Mapper",
            RomMapper::OversizeBNROM => "Oversize BNROM",
            RomMapper::NTDEC2722and2752 => "NTDEC 2722 (Sub. 0) or 2752 (Sub. 1)",
            RomMapper::Caltron6in1 => "Caltron 6-in-1 Multicart Mapper",
            RomMapper::HackedFDS => "FDS to Cartridge hack Mapper",
            RomMapper::TONYI => "TONY-I",
            RomMapper::MMC3Multicart => "MMC3 Multicart Mapper (Super Big 7-in-1)",
            RomMapper::MulticartGA23C => "Multicart using GA23C",
            RomMapper::RumbleStation => "Rumble Station Mapper",
            RomMapper::MMC3Multicart2 => {
                "MMC3 Multicart Mapper (Super Spike V'Ball + Nintendo World Cup)"
            }
            RomMapper::TC0690 => "Taito TC0690",
            RomMapper::MMC3Multicart3 => "MMC3 Multicart Mapper (Super HIK 4-in-1)",
            RomMapper::SMB2N32 => "N-32 conversion of SMB2J Mapper",
            RomMapper::BallGames11in1 => "Multicart Mapper (11-in-1 Ball Games)",
            RomMapper::Realtec8213 => "Realtec 8213",
            RomMapper::Supervision16in1 => "Multicart Mapper (Supervision 16-in-1)",
            RomMapper::NovelDiamond201Alias => {
                "Mapper 201 Multicart Mapper for Novel Diamond 9999999-in-1"
            }
            RomMapper::BTLMario1 => "BTL-MARIO1-MALEE2",
            RomMapper::UnlSMB3 => "Unlicensed SMB3 Mapper",
            RomMapper::MulticartMapper4 => "Multicart Mapper (GK 47-in-1, 6-in-1 (SuperGK))",
            RomMapper::NROMMulticart => "NROM-/CNROM based Multicart Mapper",
            RomMapper::BMCT3H53 => "BMC-T3H53 and BMC-D1038 Multicart Mappers",
            RomMapper::NROM128ResetMulticart => "NROM-128 Multicart Mapper (Reset Based 4-in-1)",
            RomMapper::MulticartMapper5 => {
                "Multicart Mapper (方塊外傳 9合1 - Tetris Family 9-in-1, HQ 高品質合卡 15-in-1 \
                 (Sub. 0) and 32-in-1 (Sub. 1))"
            }
            RomMapper::MulticartMapper6 => "Multicart Mapper (Super 700-in-1)",
            RomMapper::NTDECMulticartTH29913 => {
                "NTDEC Multicart Mapper (Powerful 250-in-1 (Sub. 0) and 82-in-1 (Sub. 1))"
            }
            RomMapper::RAMBO1 => "Tengen RAMBO-1",
            RomMapper::H3001 => "Irem H3001",
            RomMapper::GxROM => "GxROM",
            RomMapper::Sunsoft3 => "Sunsoft-3",
            RomMapper::Sunsoft4 => "Sunsoft-4",
            RomMapper::SunsoftFME7 => "Sunsoft FME-7, 5A, or 5B",
            RomMapper::BandaiMapperFamilyMat => "Bandai Family Mat Mapper",
            RomMapper::Codemasters => "Codemasters Camerica Mapper",
            RomMapper::JF17 => "Jaleco JF-17",
            RomMapper::VRC3 => "Konami VRC3",
            RomMapper::Waixing43393 => "Waixing 43-393/43-406/860908C",
            RomMapper::VRC1 => "Konami VRC1",
            RomMapper::Namcot108FamCHR128Coarse => "Namcot 108 Family (Coarse Banking variant)",
            RomMapper::NapoleonSenki => "Napoleon Senki Mapper",
            RomMapper::HolyDiver => "Holy Diver and Uchuusen - Cosmo Carrier Mapper",
            RomMapper::NINA03and06 => "NINA-03 or NINA-06",
            RomMapper::X1005 => "Taiko X1-005",
            RomMapper::NTDECSuperGun => "NTDEC Super Gun Mapper",
            RomMapper::X1017 => "Taiko X1-017",
            RomMapper::Cony => "Cony",
            RomMapper::PCSMB2J => "PC-SMB2J",
            RomMapper::VRC7 => "Konami VRC7",
            RomMapper::JF13 => "Jaleco JF-13",
            RomMapper::J87 => "J87",
            RomMapper::Namcot108FamCHR128PPU => {
                "Namcot 108 Family (PPU A12 to CHR ROM A16 variant)"
            }
            RomMapper::Sunsoft2B3 => "Sunsoft-2 on Sunsoft-3 Board",
            RomMapper::JingtaiASICInhibited => "晶太 (Jīngtài) ASIC (inhibited)",
            RomMapper::JY830623C => "JY830623C or YY840238C (Sub 0.), or EJ-006-1 (Sub. 0)",
            RomMapper::Namcot108Fam => "Namcot 108 Family (generic)",
            RomMapper::JF17Alt => "Jaleco JV-17 with alternate PRG Setup",
            RomMapper::Sunsoft2B3R => "Sunsoft-2 on Sunsoft-3R Board",
            RomMapper::Namcot108FamVariableArrangement => {
                "Namcot 108 Family (with changeable Nametable Arrangement)"
            }
            RomMapper::BandaiOekaKids => "Bandai Oeka Kids Tablet Mapper",
            RomMapper::TAMS1 => "Irem TAM-S1",
            RomMapper::GBCPort => "GBC-to-NES Porting Mapper",
            RomMapper::VsSystemCNROM => "Vs. System CNROM Mapper",
            RomMapper::NesticleMMC3Hack => "Nesticle MMC3 hack Mapper",
            RomMapper::J87Reverse => "J87 Reversed",
            RomMapper::QuietustDripAlt => "Quietust Drip alternative Mapper",
            RomMapper::DokiDokiPanicPirate => "Doki Doki Panic FDS Pirate Mapper",
            RomMapper::Pegasus5in1 => "PEGASUS 5 IN 1 Multicart Mapper",
            RomMapper::SMB3Bootleg => "SMB3 Bootleg Mapper",
            RomMapper::MagicDragon => "Magicseries Magic Dragon Mapper",
            RomMapper::FDSCartridgeMapper => "FDS-to-Cartridge Mapper",
            RomMapper::GreatWall => "Sachen The Great Wall legacy Mapper",
            RomMapper::HoneyPeach => "Sachen Honey Peach legacy Mapper",
            RomMapper::GTROMorMMC1Unserial => "GTROM or Non-serialized MMC1 variant",
            RomMapper::NTDECScrambled => "NTDEC Mapper similar to Namcot 108 Family",
            RomMapper::NTD8 => "HES NTD-8",
            RomMapper::MMC3CloneScrambled => "MMC3 Clone with scrambled registers",
            RomMapper::KashengMMC3 => "卡聖 (Kǎshèng) SFC-02B/-03/-004",
            RomMapper::SOMARIP => "哥德 (Gouder) SOMARI-P",
            RomMapper::FutureMediaMapper => "Future Media Mapper",
            RomMapper::LH15 => "LH15",
            RomMapper::A9711andA9713 => "卡聖 (Kǎshèng) A9711 or A9713",
            RomMapper::JY043 => "JY043",
            RomMapper::H2288 => "卡聖 (Kǎshèng) H2288",
            RomMapper::SuperGameMegaTypeIIIMapper => {
                "Super Game Mega Type III pirate arcade Mapper"
            }
            RomMapper::LH32 => "LH32 FDS Mapper",
            RomMapper::TEC9719orING003CorING022RepurposedLines => "TEC9719, ING003C or ING-022",
            RomMapper::DoubleDragonIIPirate => "Double Dragon II Pirate Mapper",
            RomMapper::T262 => "T-262 Multicart Mapper",
            RomMapper::DuplicateNROMMulticart => {
                "Duplicate of Mapper 58 (NROM-/CNROM based Multicart Mapper)"
            }
            RomMapper::Alternative331 => {
                "Alternative assignment of Mapper 331 (MMC3 Multicart Mapper)"
            }
            RomMapper::Alternative205 => {
                "Alternative assignment of Mapper 205 (NS03 7-in-1 Mapper)"
            }
            RomMapper::TXC22Fam => "TXC 01-22 Family",
            RomMapper::SachenJovialMasterChu => {
                "Sachen (Jovial Race and 盜帥 Master Chu and the Drunkard Hu). "
            }
            RomMapper::T4A54A => "MMC3 Multicart T4A54A, WX-KB4K, or BS-5652",
            RomMapper::Sachen8259AOld => "Sachen 8259A (Old. Assignment moved to Mapper 141)",
            RomMapper::Sachen3011 => "Sachen 3011",
            RomMapper::Sachen8259D => "Sachen 8259D",
            RomMapper::Sachen8259B => "Sachen 8259B",
            RomMapper::Sachen8259C => "Sachen 8259C",
            RomMapper::JF11orJF14 => "Jaleco JF11 or JF14",
            RomMapper::Sachen8259A => "Sachen 8259A",
            RomMapper::KS7032 => "Kaiser KS-7032",
            RomMapper::NROMCopyProt => "NROM Variant (Copy Protected)",
            RomMapper::MMC3Multicart7 => "MMC3 Multicart Mapper",
            RomMapper::NS037in0 => "NS03 7-in-1 Mapper",
            RomMapper::DeathRaceMapper => "Death Race Mapper",
            RomMapper::SA72007 => "Sachen SA-72007",
            RomMapper::Sachen3015 => "Sachen 3015",
            RomMapper::Sachen3018 => "Sachen 3018",
            RomMapper::SA008A => "Sachen SA-008-A",
            RomMapper::SA0036 => "Sachen SA-0036",
            RomMapper::SA015 => "Sachen SA-015",
            RomMapper::VsSystemVRC1 => "VRC1 in Vs. System",
            RomMapper::BandaiFamilyMatPlus => "Bandai Family Mat Mapper with arrangement control",
            RomMapper::FCGWithLZ93D508KiBWRAM => "Bandai FCG with LZ93D508 and 8KiB of WRAM",
            RomMapper::NAMCOT3453 => "Namcot-3453",
            RomMapper::DIS23C01 => "DAOU ROM Controller DIS23C01 DAOU 245",
            RomMapper::FCGDetachedJointRom => "Bandai Detached Joint ROM System",
            RomMapper::Tengen800037 => "Tengen 800037",
            RomMapper::FCGWithLZ93D50EEPROM => "Bandai FCG with LZ93D50 with EEPROM",
            RomMapper::JingtaiASICDuplicate => {
                "晶太 (Jīngtài) ASIC (inhibited) (Duplicate Assignment of Mapper 90)"
            }
            RomMapper::HanjukuHeroMMC1 => "Hanjuku Hero (Duplicate Assignment of MMC1)",
            RomMapper::FS304 => "外星 (Wàixīng) FS304",
            RomMapper::FC001 => "南晶 (Nánjīng) FC-001",
            RomMapper::CY20003 => "燕城 (Yànchéng) cy2000-3",
            RomMapper::MMC2MMC3Mashup => "MMC3 and MMC2 Hybrid",
            RomMapper::IncorrectSuborLearning => {
                "Incorrect version of 小霸王 中英文电脑学习机 IV (Subor
                Chinese and English Computer Learning Machine IV
                )"
            }
            RomMapper::SuborLearning => {
                "小霸王 中英文电脑学习机 IV (Subor
                Chinese and English Computer Learning Machine IV
                ) Mapper"
            }
            RomMapper::Racermate => "Racermate Challenge 2 Mapper",
            RomMapper::Yuxing => "Yuxing Mapper",
            RomMapper::ShikoGameSyu => "Shiko Game Syu Mapper",
            RomMapper::Floppy1 => "步步高 (Bùbùgāo, BBK) Floppy-1 and SC-98",
            RomMapper::P4070 => "Super Mega P-4070",
            RomMapper::IdeaTek => "Idea-Tek Mapper",
            RomMapper::NROMMulticartShuffled => "NROM Multicart Mapper with shuffled bits",
            RomMapper::Kaiser15in1 => "Kaiser 15-in-1 Multicart Mapper",
        };

        let mapper_num: u16 = (*self).into();

        write!(f, "{str} (INes Mapper {:03})", mapper_num)
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
    pub fn get_for_header(header: &[u8], name: String) -> Result<Self, ParseError> {
        let rom_type = RomFile::get_rom_type(header, true)?;
        let mut file = rom_type.parse(header, Some(&name))?;
        file.format_name = rom_type.get_name().to_string();
        Ok(file)
    }

    fn range_all_zeros(arr: &[u8], start: usize, end: usize) -> bool {
        if start > end || end > arr.len() {
            return false;
        }
        arr[start..end].iter().all(|&x| x == 0)
    }

    fn get_rom_type(rom: &[u8], header_only: bool) -> Result<Box<dyn RomParser>, ParseError> {
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
                && ((prg_rom_size as usize + chr_rom_size as usize) < rom.len() || header_only)
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
    pub fn load(
        data: &mut [u8],
        name: Option<&String>,
        use_db: bool,
        nes: Option<&Nes>,
    ) -> Result<RomFile, ParseError> {
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let full_hash: [u8; 32] = hasher.finalize().into();

        let mut hasher = Sha256::new();
        hasher.update(&data[16..]);
        let headerless_hash: [u8; 32] = hasher.finalize().into();

        if use_db && let Some(nes) = nes {
            let rom_db = nes.get_rom_db();
            let primary_lookup = rom_db.get_entry(&full_hash);

            if primary_lookup.is_none()
                && let Some(lookup) = rom_db.get_entry_by_headerless(&headerless_hash)
                && let Some(header) = &lookup.header
            {
                data[..header.len()].copy_from_slice(header);
            }
        }

        let rom_type = RomFile::get_rom_type(data, false)?;
        let mut rom_file = rom_type.parse(data, name)?;
        rom_file.format_name = rom_type.get_name().to_string();
        rom_file.data = data.to_vec();

        rom_file.data_checksum = full_hash;
        rom_file.checksum_headerless = headerless_hash;

        if use_db && let Some(nes) = nes {
            let rom_db = nes.get_rom_db();
            let headerless = rom_db.get_entry_by_headerless(&headerless_hash);

            if let Some(entry) = headerless {
                rom_file.name = Some(entry.name.clone());
                rom_file.original_name = entry.orig_name.clone();
            }
        }

        Ok(rom_file)
    }

    /// Extracts the PRG ROM region as a read-only [`Memory`] device.
    ///
    /// This is used internally to populate the CPU memory map at addresses
    /// `$8000`-`$FFFF`.
    #[doc(hidden)]
    pub fn get_prg_rom(&self) -> Memory {
        let mut rom = Memory::new(self.prg_memory.prg_rom_size as usize, false);

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
    pub fn get_chr_rom(&self) -> Option<Memory> {
        if self.chr_memory.chr_rom_size == 0 {
            return None;
        }

        let mut rom = Memory::new(self.chr_memory.chr_rom_size as usize, false);

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
    pub fn get_prg_ram(&self) -> Option<Memory> {
        if self.prg_memory.prg_ram_size > 0 {
            let mut ram = Memory::new(self.prg_memory.prg_ram_size as usize, true);

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
    pub fn get_nametable_arrangement(&self) -> NametableArrangement {
        match self.hardwired_nametable_layout {
            true => NametableArrangement::Horizontal,
            false => NametableArrangement::Vertical,
        }
    }
}

impl From<&RomFile> for RomFile {
    fn from(rom: &RomFile) -> Self { rom.clone() }
}

impl TryFrom<(&mut [u8], Option<&Nes>)> for RomFile {
    type Error = ParseError;

    fn try_from((data, nes): (&mut [u8], Option<&Nes>)) -> Result<Self, Self::Error> {
        RomFile::load(data, None, true, nes)
    }
}

impl TryFrom<(&mut [u8], &String, bool, Option<&Nes>)> for RomFile {
    type Error = ParseError;

    fn try_from(
        (data, name, use_db, nes): (&mut [u8], &String, bool, Option<&Nes>),
    ) -> Result<Self, Self::Error> {
        RomFile::load(data, Some(name), use_db, nes)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl TryFrom<(&String, bool, Option<&Nes>)> for RomFile {
    type Error = ParseError;

    fn try_from((path, use_db, nes): (&String, bool, Option<&Nes>)) -> Result<Self, Self::Error> {
        use std::fs::File;
        use std::io::Read;

        let mut data = Vec::new();
        File::open(path)?.read_to_end(&mut data)?;

        RomFile::load(&mut data, Some(path), use_db, nes)
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
            extended_console_type: self.extended_console_type.map(ExtendedConsoleType::from),
            vs_system_hardware_type: self.vs_system_hardware_type.map(VsHardwareType::from),
            vs_system_ppu_type: self.vs_system_ppu_type.map(VsSystemPpuType::from),
            timing_region: RomTimingRegion::from(self.rom_timing_region),
            console_type: ConsoleType::from(self.console_type),
            hardwired_nametable_layout: self.hardwired_nametable_layout,
            is_battery_backed: self.is_battery_backed,
            trainer_present: self.trainer_present,
            alternative_nametables: self.alternative_nametables,
            submapper_number: self.submapper_number,
            data_checksum: [0; 32],
            checksum_headerless: [0; 32],
            data: Vec::new(),
            original_name: None,
            format_name: String::new(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use crate::emulation::rom::{ExpansionDevice, RomBuilder, RomMapper};

    #[test]
    fn repr_enums_serialize_as_numbers() {
        let serialized = serde_json::to_string(&ExpansionDevice::StandardController)
            .expect("failed to serialize expansion device");
        assert_eq!(serialized, "1");

        let serialized =
            serde_json::to_string(&RomMapper::MMC1).expect("failed to serialize rom mapper");
        assert_eq!(serialized, "1");
    }

    #[test]
    fn repr_enums_deserialize_unknown_values() {
        let expansion: ExpansionDevice =
            serde_json::from_str("200").expect("failed to deserialize expansion device");
        assert_eq!(expansion, ExpansionDevice::Unknown(200));

        let mapper: RomMapper = serde_json::from_str("9999").expect("failed to deserialize mapper");
        assert_eq!(mapper, RomMapper::Unknown(9999));
    }

    #[test]
    fn rom_file_enum_fields_use_repr_values_in_json() {
        let rom = RomBuilder::new()
            .mapper_number(1)
            .default_expansion_device(1)
            .console_type(1)
            .cpu_ppu_timing(2)
            .vs_system_hardware_type(Some(5))
            .vs_system_ppu_type(Some(3))
            .extended_console_type(Some(0xC))
            .build();

        let json = serde_json::to_string(&rom).expect("failed to serialize rom");
        assert!(json.contains("\"mapper\":1"));
        assert!(json.contains("\"default_expansion_device\":1"));
        assert!(json.contains("\"console_type\":1"));
        assert!(json.contains("\"timing_region\":2"));
        assert!(json.contains("\"vs_system_hardware_type\":5"));
        assert!(json.contains("\"vs_system_ppu_type\":3"));
        assert!(json.contains("\"extended_console_type\":12"));
    }
}
