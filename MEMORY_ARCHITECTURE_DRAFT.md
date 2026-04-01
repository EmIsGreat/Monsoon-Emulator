# Memory Architecture Overhaul (Summary + Rust Draft)

## Scope

This document is intentionally **design-only**.  
It describes the desired architecture and a rough Rust draft without implementing behavior.

---

## Desired Architecture Summary

### 1) Core idea: model buses, not ownership of memory slices

The emulator should treat memory access like hardware bus activity:

- CPU places an address/data on the **CPU bus**
- PPU places an address/data on the **PPU bus**
- Connected devices decide how to respond

This replaces direct cross-component memory ownership assumptions with explicit routing.

### 2) Single source of truth for shared devices

Shared resources (cartridge PRG/CHR, mapper registers, nametable behavior, DMA/NMI lines) should be owned by a central board/interconnect layer.

- CPU and PPU become clients of that interconnect
- Cartridge owns CHR-RAM/ROM and PRG-RAM/ROM access rules
- Side effects are emitted as signals/events rather than ad-hoc shared mutable access

### 3) Explicit side-effect propagation

Bus operations can trigger effects beyond returning a byte:

- CPU write to `$4014` => DMA request
- PPU status/ctrl transitions => NMI edge/latch behavior
- Mapper writes => bank switching affecting both CPU and PPU views

These should be represented through typed outcomes/signals.

### 4) Cartridge/mapper as routing authority for cart space

Instead of mapping raw `Memory` objects into CPU/PPU independently:

- CPU cart address range routes to mapper (`cpu_read/cpu_write`)
- PPU pattern-table/cart ranges route to mapper (`ppu_read/ppu_write`)
- CHR-RAM sharing is naturally solved because mapper owns that state

### 5) Keep timing ownership at top-level coordinator

A top-level coordinator (board/nes core) remains responsible for master-cycle sequencing and for sampling/consuming pending signals (NMI/IRQ/DMA) at correct timing points.

---

## Rough Rust Draft (Interfaces/Types)

> Note: This is intentionally a draft API sketch, not implementation code.

```rust
// ---- Bus operation context ----

#[derive(Debug, Clone, Copy)]
pub enum BusMaster {
    Cpu,
    Ppu,
}

#[derive(Debug, Clone, Copy)]
pub struct BusRead {
    pub master: BusMaster,
    pub addr: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct BusWrite {
    pub master: BusMaster,
    pub addr: u16,
    pub data: u8,
}

// ---- Side effects/signals ----

#[derive(Debug, Clone, Copy)]
pub enum Signal {
    NmiRise,
    NmiFall,
    IrqAssert,
    IrqClear,
    DmaStart { page: u8 },
}

#[derive(Debug, Default, Clone)]
pub struct SignalLatch {
    pub pending: Vec<Signal>,
}

impl SignalLatch {
    pub fn push(&mut self, signal: Signal);
    pub fn drain(&mut self) -> impl Iterator<Item = Signal>;
}

// ---- Bus read/write outcomes ----

#[derive(Debug, Clone, Copy)]
pub struct ReadResult {
    pub data: u8,
}

#[derive(Debug, Default)]
pub struct WriteResult;

// ---- Device-facing CPU/PPU bus traits ----

pub trait CpuBus {
    fn read(&mut self, addr: u16) -> ReadResult;
    fn write(&mut self, addr: u16, data: u8) -> WriteResult;
}

pub trait PpuBus {
    fn read(&mut self, addr: u16) -> ReadResult;
    fn write(&mut self, addr: u16, data: u8) -> WriteResult;
}

// ---- Mapper/cartridge split ----

pub trait Mapper {
    fn cpu_read(&mut self, addr: u16) -> u8;
    fn cpu_write(&mut self, addr: u16, data: u8);
    fn ppu_read(&mut self, addr: u16) -> u8;
    fn ppu_write(&mut self, addr: u16, data: u8);
    fn poll_irq(&self) -> bool;
}

pub struct Cartridge {
    pub mapper: Box<dyn Mapper>,
    // PRG/CHR backing storage and metadata live behind mapper ownership.
}

impl Cartridge {
    pub fn cpu_read(&mut self, addr: u16) -> u8;
    pub fn cpu_write(&mut self, addr: u16, data: u8);
    pub fn ppu_read(&mut self, addr: u16) -> u8;
    pub fn ppu_write(&mut self, addr: u16, data: u8);
}

// ---- Board/interconnect (shared hardware owner) ----

pub struct Board {
    // core shared state
    pub cartridge: Cartridge,
    pub cpu_ram: Box<[u8; 0x800]>,
    pub ppu_nametable_ram: Box<[u8; 0x800]>,
    pub ppu_palette_ram: Box<[u8; 0x20]>,
    pub oam: Box<[u8; 0x100]>,

    // control/signal state
    pub signals: SignalLatch,
    pub cpu_open_bus: u8,
    pub ppu_open_bus: u8,
}

impl CpuBus for Board {
    fn read(&mut self, addr: u16) -> ReadResult;
    fn write(&mut self, addr: u16, data: u8) -> WriteResult;
}

impl PpuBus for Board {
    fn read(&mut self, addr: u16) -> ReadResult;
    fn write(&mut self, addr: u16, data: u8) -> WriteResult;
}

// ---- CPU/PPU consume interfaces, not shared ownership internals ----

pub struct Cpu {
    // registers/state...
}

impl Cpu {
    pub fn step<B: CpuBus>(&mut self, bus: &mut B);
}

pub struct Ppu {
    // registers/state...
}

impl Ppu {
    pub fn step<B: PpuBus>(&mut self, bus: &mut B);
}

// ---- Top-level NES coordinator ----

pub struct NesCore {
    pub cpu: Cpu,
    pub ppu: Ppu,
    pub board: Board,
    pub master_cycle: u128,
}

impl NesCore {
    pub fn step_master_cycle(&mut self);
    fn apply_signals(&mut self); // consume latched DMA/NMI/IRQ updates
}
```

---

## Recommended Migration Shape (No Implementation Here)

1. Introduce bus traits and board skeleton in parallel with current memory map.
2. Route CPU accesses through `CpuBus`.
3. Route PPU accesses through `PpuBus`.
4. Move cartridge-visible ranges to mapper-owned routing.
5. Replace direct cross-references with signal latches.
6. Remove obsolete direct shared-memory paths after behavior parity is reached.

