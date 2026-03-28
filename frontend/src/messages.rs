use monsoon_core::emulation::ppu_util::EmulatorFetchable;
use monsoon_core::emulation::rom::RomFile;
use monsoon_core::emulation::savestate::SaveState;

use crate::frontend::messages::LoadedRom;

/// Message types for communication between the frontend and emulator.
///
/// This module defines the message protocol for bidirectional communication:
/// - `FrontendMessage`: Commands sent from the frontend to the emulator
/// - `EmulatorMessage`: Notifications sent from the emulator to the frontend
/// - `ControllerEvent`: Input events for the emulated console
///
/// The message-based architecture provides clean separation between the
/// frontend and emulation logic, enabling future threading and remote control
/// features.
///
/// Messages sent from the frontend to the emulator
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum FrontendMessage {
    /// Request to quit the emulator
    Quit,
    /// Controller input events
    ControllerInput(ControllerEvent),
    /// Request to reset the console
    Reset,
    Power,
    PowerOff,
    /// Request to step one frame
    StepFrame,
    RequestDebugData(EmulatorFetchable),
    LoadRom((LoadedRom, String)),
    WritePpu(u16, u8),
    WriteCpu(u16, u8),
    CreateSaveState(SaveType),
    LoadSaveState(Box<SaveState>),
    StepPpuCycle,
    StepCpuCycle,
    StepMasterCycle,
    StepScanline,
}

#[derive(Debug, Default, Eq, PartialEq, Copy, Clone, Hash)]
pub enum SaveType {
    #[default]
    Manual,
    Quicksave,
    Autosave,
}

/// Controller input events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControllerEvent {
    Left,
    Right,
    Up,
    Down,
    Start,
    Select,
    A,
    B,
}

/// Messages sent from the emulator to the frontend
#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub enum EmulatorMessage {
    /// Raw frame data as u16 palette indices (frontend applies RGB palette)
    FrameReady(Vec<u16>),
    /// Emulator has stopped/quit
    Stopped,
    DebugData(EmulatorFetchable),
    SaveState(Box<SaveState>, SaveType),
    RomLoaded(Option<(RomFile, LoadedRom)>),
}
