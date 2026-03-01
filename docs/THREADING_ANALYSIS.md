# Threading Analysis: Emulator Backend in a Separate Thread

> **Status: IMPLEMENTED.** The changes described below have been applied.
> See the implementation commits for details.

This document describes the architecture for running the emulator backend
(`ChannelEmulator` + `Nes`) in a dedicated thread, fully decoupled from the
egui frontend via crossbeam channels.

---

## Table of Contents

1. [Current Architecture](#current-architecture)
2. [Send/Sync Blockers](#sendsync-blockers)
3. [Required Core Crate Changes](#required-core-crate-changes)
4. [Required Frontend Crate Changes](#required-frontend-crate-changes)
5. [Message Protocol Changes](#message-protocol-changes)
6. [WASM Considerations](#wasm-considerations)
7. [Performance Considerations](#performance-considerations)
8. [Implementation Phases](#implementation-phases)

---

## Current Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│                     EguiApp::update()  (UI thread)                   │
│                                                                      │
│  1. handle_keyboard_input()      ← reads egui input                  │
│  2. channel_emu.process_messages()  ← drains FrontendMessage queue   │
│  3. channel_emu.execute_frame()  ← runs Nes::step_frame() N times   │
│  4. process_messages()           ← drains EmulatorMessage queue      │
│  5. render UI                                                        │
└──────────────────────────────────────────────────────────────────────┘
```

Today, `ChannelEmulator` lives **inside** `EguiApp` and runs on the **same
thread** as the UI. The crossbeam channels exist as an abstraction layer but do
not cross a thread boundary. Steps 2–3 above block the UI thread while the
emulator runs.

### Key types

| Type | Location | Role |
|------|----------|------|
| `Nes` | `core/src/emulation/nes.rs` | Top-level emulator: orchestrates CPU, PPU, memory |
| `Cpu` | `core/src/emulation/cpu.rs` | MOS 6502 CPU |
| `Ppu` | `core/src/emulation/ppu.rs` | 2C02 PPU (rendering) |
| `ChannelEmulator` | `frontend/src/channel_emu.rs` | Message-passing wrapper around `Nes` |
| `FrontendMessage` | `frontend/src/messages.rs` | Frontend → emulator commands |
| `EmulatorMessage` | `frontend/src/messages.rs` | Emulator → frontend notifications |

### Ownership graph (simplified)

```
Nes
 ├── cpu: Cpu
 │    ├── memory: MemoryMap          (owns RAM, ROM, MirrorMemory, …)
 │    │    └── PpuRegisters          ← holds Rc<RefCell<Ppu>>
 │    └── ppu: Option<Rc<RefCell<Ppu>>>
 └── ppu: Rc<RefCell<Ppu>>
```

Both `Nes.ppu` and `Cpu.ppu` (and `PpuRegisters` inside the CPU memory map)
point to the **same** `Ppu` instance via `Rc<RefCell<Ppu>>`. This shared
ownership is the fundamental reason the emulator is not `Send`.

---

## Send/Sync Blockers

For `ChannelEmulator` to move to a separate thread it must be `Send`. Today it
is **not** `Send` because `Nes` is not `Send`, which traces to:

### 1. `Rc<RefCell<Ppu>>` — the primary blocker

`Rc` is not `Send` (single-threaded reference counting). Three places hold a
clone of the same `Rc<RefCell<Ppu>>`:

| Location | Field | File |
|----------|-------|------|
| `Nes` | `ppu: Rc<RefCell<Ppu>>` | `nes.rs:66` |
| `Cpu` | `ppu: Option<Rc<RefCell<Ppu>>>` | `cpu.rs:48` |
| `PpuRegisters` (inside CPU MemoryMap) | `ppu: Rc<RefCell<Ppu>>` | `mem/ppu_registers.rs:9` |

### 2. `Cell<T>` fields inside `Ppu`

`Ppu` uses `Cell<u8>`, `Cell<bool>`, `Cell<Option<u8>>`, and `Cell<OpenBus>`
for interior mutability (fields like `status_register`, `nmi_requested`,
`write_latch`, `vbl_clear_scheduled`, `open_bus`). `Cell<T>` is `Send` when
`T: Send`, so these are **not** blockers for a move to another thread. However,
they are **not** `Sync`, which means the PPU cannot be shared across threads
behind a bare `Arc` — it would need `Arc<Mutex<Ppu>>` or similar.

### 3. `RefCell<u8>` in `ApuRegisters`

`ApuRegisters` uses `RefCell<u8>` for `status` and `read_counter`
(`mem/apu_registers.rs:8,11`). `RefCell` is `Send` but not `Sync`, so this is
not a send blocker but is relevant if any shared-access pattern is considered.

### Summary

The **only** obstacle to making `Nes` (and therefore `ChannelEmulator`) `Send`
is the `Rc<RefCell<Ppu>>` shared between `Nes`, `Cpu`, and `PpuRegisters`.

---

## Required Core Crate Changes

There are two viable strategies for eliminating the `Rc<RefCell<Ppu>>`.

### Strategy A: Replace `Rc<RefCell<Ppu>>` with `Arc<Mutex<Ppu>>`

This is the most direct change and preserves the current shared-ownership
architecture.

**Changes:**

1. **`nes.rs`**: Change `ppu: Rc<RefCell<Ppu>>` → `ppu: Arc<Mutex<Ppu>>`
2. **`cpu.rs`**: Change `ppu: Option<Rc<RefCell<Ppu>>>` → `ppu: Option<Arc<Mutex<Ppu>>>`
3. **`ppu_registers.rs`**: Change `ppu: Rc<RefCell<Ppu>>` → `ppu: Arc<Mutex<Ppu>>`
4. **All `.borrow()` / `.borrow_mut()` calls** → `.lock().unwrap()`
   (roughly 30–40 call sites across `nes.rs`, `cpu.rs`, `ppu_registers.rs`,
   and `savestate.rs`)
5. **`Rc::new(RefCell::new(Ppu::…))`** → **`Arc::new(Mutex::new(Ppu::…))`**
   (in `Nes::default()`, `Nes::power_off()`, `Nes::load_state()`)

**Pros:**
- Minimal structural change — keeps the same ownership topology
- All existing code paths remain logically identical

**Cons:**
- `Mutex` is heavier than `RefCell` (~25 ns lock overhead per acquisition).
  The PPU is accessed on nearly every master clock cycle (every 4th cycle for
  PPU step, every 12th for CPU step via `PpuRegisters`), so this adds
  measurable overhead to the hot loop. The core runs ~357,000 master cycles per
  frame, with thousands of mutex acquisitions per frame.
- Deadlock risk if lock ordering is not careful (though current code never holds
  the borrow across another borrow, so mechanical replacement should be safe)

**Mitigation for performance:**
- Consider using `parking_lot::Mutex` which has lower overhead than
  `std::sync::Mutex` and does not require `.unwrap()`.
- Since the PPU is **only accessed from the emulator thread** (the frontend
  never touches `Nes` directly once threaded), a `Mutex` is technically
  unnecessary at runtime — the lock would never be contended. The cost is
  purely the syscall overhead.

### Strategy B: Remove shared ownership — give `Cpu` a borrowed reference

Refactor so the CPU does not own a reference to the PPU. Instead, the `Nes`
step loop passes PPU access to the CPU on each cycle.

**Changes:**

1. Remove `Cpu.ppu` field entirely
2. Change `Cpu::step()` to accept `&mut Ppu` or a trait object
3. Remove `PpuRegisters` from the memory map; instead, intercept reads/writes
   to `$2000`–`$3FFF` in `Cpu::step()` and forward them to the PPU argument
4. `Nes` becomes the sole owner of `Ppu` (plain `Ppu`, no wrapper)

**Pros:**
- Zero overhead from reference counting or locking
- `Nes` becomes trivially `Send` (all fields are owned, no shared references)
- Cleaner ownership model

**Cons:**
- Significant refactor of CPU memory access path — every memory read/write
  currently goes through `MemoryMap`, which dispatches to `PpuRegisters`. This
  dispatch would need to be restructured.
- CPU DMA handling (OAM DMA at `$4014`) also needs PPU access, so the
  borrow-passing gets more complex
- More test churn

### Recommendation

**Strategy A** (Arc<Mutex>) is recommended for the initial threading milestone
because it is a mechanical transformation with low risk. Strategy B can be
pursued afterward as a performance optimization if profiling shows the mutex
overhead is significant.

---

## Required Frontend Crate Changes

### 1. Spawn the emulator thread

Today in `EguiApp::new()`, `ChannelEmulator::new()` returns the emulator plus
the channel endpoints. The change is to spawn a thread that runs the emulator
loop:

```rust
// Before (single-threaded):
let (mut channel_emu, tx_to_emu, rx_from_emu) = ChannelEmulator::new(nes);
// channel_emu stored in EguiApp, called every frame

// After (threaded):
let (channel_emu, tx_to_emu, rx_from_emu) = ChannelEmulator::new(nes);
std::thread::spawn(move || {
    channel_emu.run();  // New blocking loop method
});
// EguiApp only keeps tx_to_emu and rx_from_emu
```

### 2. New `ChannelEmulator::run()` method

Add a blocking event loop to `ChannelEmulator` that:

1. Waits for messages from the frontend
2. Runs emulation frames at the correct cadence
3. Sends results back via the existing channel

```rust
impl ChannelEmulator {
    /// Blocking emulator loop. Call from a dedicated thread.
    pub fn run(mut self) {
        let target_frame_time = FRAME_DURATION; // ~16.67ms
        loop {
            // Process all pending messages
            if let Err(_) = self.process_messages() {
                break; // Quit was received
            }
            // Execute one frame
            if let Err(e) = self.execute_frame() {
                eprintln!("Emulator error: {}", e);
                break;
            }
            // Rate-limit to ~60 FPS (or respect speed config)
            // This timing control moves from the frontend to here
        }
    }
}
```

### 3. Remove `channel_emu` from `EguiApp`

`EguiApp` should no longer hold `ChannelEmulator`. It should only hold:

```rust
pub struct EguiApp {
    // No more channel_emu field
    pub(crate) to_emulator: Sender<FrontendMessage>,
    pub(crate) from_emulator: Receiver<EmulatorMessage>,
    // ... rest of UI state unchanged
}
```

### 4. Move frame timing to the emulator thread

Currently, `EguiApp::update_emu_textures()` manages an accumulator-based
timing loop that calls `channel_emu.execute_frame()` multiple times per UI
frame. With threading:

- **Emulator thread**: Runs frames at its own cadence (tied to speed config).
  The speed setting must be communicated via a new `FrontendMessage` variant
  (e.g., `SetSpeed(AppSpeed)`).
- **UI thread**: Simply reads the latest `FrameReady` from the channel and
  updates textures. If multiple frames arrive between UI updates, it can skip
  to the latest one.

### 5. Controller input latency

Currently, controller input is polled once per UI frame and sent via
`FrontendMessage::ControllerInput`. With threading, this still works — the
emulator thread polls its `from_frontend` receiver before each frame. Latency
is bounded by the channel transit time (sub-microsecond for crossbeam
unbounded channels).

The `input` field on `ChannelEmulator` (which accumulates button presses via
bitwise OR) should be **reset to 0 after each frame**, then re-filled from
the queue before the next frame. This prevents sticky buttons.

### 6. Direct `Nes` access from UI (debug/savestate)

A few operations currently access `channel_emu.nes` directly from the UI
thread. These must go through the channel protocol instead:

- **Autosave on focus loss** (`check_focus_autosave`): Already sends
  `CreateSaveState` via messages — no change needed.
- **Any future direct Nes access**: Must be avoided; all interaction goes
  through `FrontendMessage`/`EmulatorMessage`.

---

## Message Protocol Changes

The existing `FrontendMessage` and `EmulatorMessage` enums are **already
well-suited** for threaded operation. The required additions are:

### New `FrontendMessage` variants

```rust
pub enum FrontendMessage {
    // ... existing variants ...

    /// Set the emulation speed. The emulator thread adjusts its frame timing.
    SetSpeed(AppSpeed),

    /// Pause/unpause emulation. The emulator thread stops/resumes frame
    /// execution but continues to process messages.
    SetPaused(bool),
}
```

### New `EmulatorMessage` variants (optional)

```rust
pub enum EmulatorMessage {
    // ... existing variants ...

    /// Emulator acknowledges a speed change (for UI confirmation)
    SpeedChanged(AppSpeed),

    /// Emulator reports an error that didn't cause a shutdown
    Error(String),
}
```

### Message trait bounds

All message types must be `Send` to cross the thread boundary. Current
`FrontendMessage` and `EmulatorMessage` **already are `Send`** because:

- `Vec<u8>`, `Vec<u16>`, `String`, `u8`, `u16` are `Send`
- `Box<SaveState>` is `Send` (SaveState contains only owned data)
- `EmulatorFetchable` contains `Box<PaletteData>`, `Option<Vec<TileData>>`,
  etc. — all `Send`
- `RomFile` is `Send` (contains `Vec<u8>`, `String`, etc.)

No changes to message types needed for `Send` compliance.

---

## WASM Considerations

`std::thread::spawn` is **not available** on `wasm32` targets. The WASM
frontend currently runs everything on a single thread and will need a
different approach:

### Option 1: Web Workers (recommended for WASM)

- Use a Web Worker as the emulator thread
- Communication via `postMessage` / `MessagePort` (analogous to channels)
- Requires the emulator core to be compiled to a separate WASM module loaded
  by the worker
- The `wasm-bindgen` + `web-sys` crates provide Web Worker bindings
- **Significant additional build infrastructure** (two WASM targets, shared
  message types serialized via `serde`)

### Option 2: Keep single-threaded on WASM

- Use `cfg(target_arch = "wasm32")` to conditionally keep the current
  single-threaded architecture on WASM
- Only native targets get the threaded emulator
- **Much simpler** — no WASM build changes

### Recommendation

Start with **Option 2** (cfg-gated threading, WASM stays single-threaded). Web
Worker support can be added later as a follow-up.

```rust
#[cfg(not(target_arch = "wasm32"))]
{
    std::thread::spawn(move || channel_emu.run());
}

#[cfg(target_arch = "wasm32")]
{
    // Keep in EguiApp, call from update() as today
    self.channel_emu = Some(channel_emu);
}
```

---

## Performance Considerations

### Channel overhead

Crossbeam unbounded channels have very low overhead:
- **Send**: ~20–50 ns (allocation for the message node)
- **Recv**: ~20–50 ns (CAS on the queue head)
- **FrameReady**: Sends a `Vec<u16>` of 61,440 elements (256×240) per frame.
  This is a pointer move (no copy), so overhead is minimal.

At 60 FPS, channel overhead is negligible (<1 µs/frame total).

### Mutex overhead (Strategy A)

If using `Arc<Mutex<Ppu>>`:
- ~25 ns per uncontended lock/unlock for `std::sync::Mutex`
- ~10 ns for `parking_lot::Mutex`
- Approximately 3,000–5,000 PPU locks per frame (PPU step + PPU register access)
- Estimated overhead: **30–125 µs/frame** (~0.2–0.8% of 16.67 ms frame budget)
- This is acceptable for a first implementation

### Frame synchronization

With the emulator on its own thread, the UI thread consumes frames
asynchronously. Two synchronization strategies:

1. **Free-running with latest-frame**: The emulator runs at its target speed.
   The UI always renders the most recent `FrameReady` message, discarding
   older ones if the UI is slower. Simple, good for uncapped speed mode.

2. **Back-pressure**: The emulator blocks after sending a frame until the
   frontend acknowledges it. This ensures exactly 1:1 frame production and
   keeps memory usage bounded, but adds latency equal to UI frame time.

**Recommendation**: Use approach 1 (free-running). For the `FrameReady` path
specifically, consider using a bounded channel (capacity 2–3) or a
latest-value swap buffer instead of the current unbounded channel, to prevent
unbounded memory growth if the UI thread falls behind. For debug data, the
current change-detection hashing approach works well across threads.

---

## Implementation Phases

### Phase 1: Make `Nes` `Send` (core crate only)

**Files changed:** `nes.rs`, `cpu.rs`, `ppu_registers.rs`, `ppu.rs` (imports
only), `savestate.rs` (borrow → lock)

1. Replace `Rc<RefCell<Ppu>>` → `Arc<Mutex<Ppu>>` in `Nes`, `Cpu`,
   `PpuRegisters`
2. Replace all `.borrow()` → `.lock().unwrap()` and `.borrow_mut()` →
   `.lock().unwrap()`
3. Replace `Rc::new(RefCell::new(…))` → `Arc::new(Mutex::new(…))`
4. Add `#[cfg(test)] static_assertions::assert_impl_all!(Nes: Send);` to
   verify `Send`
5. Run existing test suite — all 335 tests must continue to pass

**Risk:** Low. Mechanical find-and-replace.
**Estimate:** ~2 hours.

### Phase 2: Add `ChannelEmulator::run()` loop (frontend crate)

**Files changed:** `channel_emu.rs`

1. Add a `run(mut self)` method with a blocking frame loop
2. Accept speed/pause control via new `FrontendMessage` variants
3. Implement frame timing (move accumulator logic from `update_emu_textures`)
4. Fix the controller input reset issue (clear `self.input` each frame)

**Risk:** Medium. Frame timing must be tested to avoid audio/video drift.
**Estimate:** ~3 hours.

### Phase 3: Thread the frontend (frontend crate)

**Files changed:** `egui_frontend.rs`, `channel_emu.rs`, message handler files

1. Spawn `ChannelEmulator` on a dedicated thread in `EguiApp::new()`
2. Remove `channel_emu` field from `EguiApp`
3. `cfg`-gate threading for native only; keep single-threaded on WASM
4. Update `update_emu_textures()` to consume frames from the channel rather
   than calling `execute_frame()` directly
5. Add `SetSpeed` / `SetPaused` message handling
6. Test on native (Linux/macOS/Windows) and WASM

**Risk:** Medium-High. UI responsiveness and frame timing need careful tuning.
**Estimate:** ~4 hours.

### Phase 4 (future): Performance optimization

1. Profile `Arc<Mutex<Ppu>>` overhead
2. If significant, pursue Strategy B (remove shared PPU ownership)
3. Consider `parking_lot::Mutex` as a drop-in replacement
4. Consider lock-free frame buffer (e.g., triple buffering)

### Phase 5 (future): WASM Web Worker support

1. Compile emulator core as a standalone WASM module
2. Load in a Web Worker
3. Bridge `FrontendMessage`/`EmulatorMessage` via `postMessage` + serde
4. Requires build infrastructure changes (`trunk` / `wasm-pack` config)
