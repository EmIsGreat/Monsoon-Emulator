# Threading Architecture Analysis

## Problem Statement

Previously, there was an attempt to split the emulation and UI loops onto separate threads, which failed because the overhead added by requiring `Arc<Mutex<Ppu>>` instead of `Rc<RefCell<Ppu>>` was significantly larger than the performance savings introduced by running the frontend on a separate thread.

## Key Insight

**The `Send` trait is only required when moving a value FROM one thread TO another thread.** If we create the `Nes` emulator **on the background thread** rather than creating it on the main thread and moving it, we don't need `Nes` to be `Send` at all!

## Current Architecture Analysis

### Why `Nes` is Not `Send`

The `Nes` struct contains:
```rust
pub(crate) ppu: Rc<RefCell<Ppu>>,
```

- `Rc<RefCell<>>` is NOT `Send` (cannot be safely transferred between threads)
- This is intentional: `Rc` is cheaper than `Arc` because it doesn't use atomic operations
- `RefCell` runtime borrow checking is faster than `Mutex` lock acquisition

### The CPU-PPU Shared Access Pattern

The PPU must be wrapped in `Rc<RefCell<>>` because:
1. The CPU needs to read/write PPU registers through memory-mapped addresses (0x2000-0x3FFF)
2. Both CPU and PPU need simultaneous access to shared state
3. This requires **interior mutability** (`RefCell`) and **shared ownership** (`Rc`)

Example from `core/src/emulation/nes.rs:101-108`:
```rust
pub fn power(&mut self) {
    self.cpu.ppu = Some(self.ppu.clone());  // Shares Rc

    self.cpu.memory.add_memory(
        0x2000..=0x3FFF,
        Memory::MirrorMemory(MirrorMemory::new(
            Box::new(Memory::PpuRegisters(PpuRegisters::new(self.ppu.clone()))),
            0x0007,
        )),
    );
    // ...
}
```

### Performance Overhead of Arc vs Rc

If we converted `Rc<RefCell<Ppu>>` to `Arc<Mutex<Ppu>>`:

1. **Atomic overhead**: Every `Arc::clone()` uses atomic operations (compare-and-swap)
2. **Mutex overhead**: Every PPU access requires lock acquisition
3. **Contention overhead**: If multiple threads actually accessed the PPU simultaneously

The PPU is borrowed VERY frequently during the tight emulation loop:
- `ppu.borrow()` / `ppu.borrow_mut()` called every frame
- Called during CPU memory-mapped register access
- Called during every PPU step operation

From `core/src/emulation/nes.rs:408-428`:
```rust
// Frequent PPU borrows during emulation
{
    let ppu = self.ppu.borrow();
    if ppu.vbl_clear_scheduled.get().is_some() {
        ppu.vbl_reset_counter.set(ppu.vbl_reset_counter.get() + 1);
        ppu.process_vbl_clear_scheduled();
    }
}

if self.ppu_cycle_counter == 4 {
    res = res.merge(self.ppu.borrow_mut().step());
    res.ppu_cycle_completed = true;
    self.ppu_cycle_counter = 0;
}
```

## Solution: Create Nes on Background Thread

### The Approach

Instead of:
```rust
// ❌ This requires Nes to be Send
let nes = Nes::default();  // Create on main thread
std::thread::spawn(move || {
    // Move nes to background thread - REQUIRES Send!
});
```

We do:
```rust
// ✅ This does NOT require Nes to be Send
std::thread::spawn(|| {
    let nes = Nes::default();  // Create ON background thread
    // nes never crosses thread boundary!
});
```

### Why This Works

1. **`Nes` is created on the background thread** - never moved from another thread
2. **Only channel messages cross thread boundaries** - they are already `Send`
3. **No Arc/Mutex needed** - keeps the fast `Rc<RefCell<>>` pattern
4. **UI thread remains responsive** - emulation runs in parallel

### Message Types Must Be Send

The channel messages that cross thread boundaries ARE `Send`:
- `FrontendMessage` (LoadRom, StepFrame, Reset, etc.) - contains only `Send` types
- `EmulatorMessage` (FrameReady, DebugData, etc.) - contains only `Send` types

```rust
pub enum FrontendMessage {
    LoadRom((LoadedRom, String)),  // Vec<u8>, String are Send
    StepFrame,                      // Unit type is Send
    Reset,                          // Unit type is Send
    ControllerInput(ControllerEvent), // Enum is Send
    // ... all variants are Send
}

pub enum EmulatorMessage {
    FrameReady(Vec<u16>),           // Vec<u16> is Send
    RomLoaded(Option<(RomFile, LoadedRom)>), // All fields are Send
    // ... all variants are Send
}
```

## Implementation: ThreadedEmulator

See `frontend/src/threaded_emu.rs` for the complete implementation.

### Architecture Diagram

```text
UI Thread                          Background Thread
─────────                          ─────────────────
  │                                      │
  ├─ Send FrontendMessage ─────────────>│
  │   (StepFrame, LoadRom, etc.)        │
  │                                      ├─ Nes::new() on thread
  │                                      ├─ process_messages()
  │                                      ├─ nes.step_frame()
  │                                      │
  │<──── Send EmulatorMessage ──────────┤
       (FrameReady, DebugData, etc.)    │
```

### Key Design Points

1. **ThreadedEmulator**: Public API, holds thread handle and sender
2. **EmulatorWorker**: Private struct running on background thread, owns `Nes`
3. **Nes never leaves background thread**: Created and used entirely on one thread
4. **Drop handler**: Gracefully shuts down thread when frontend closes

### Benefits

- **No Arc/Mutex overhead**: Keeps fast `Rc<RefCell<>>` performance
- **UI responsiveness**: Emulation doesn't block the UI thread
- **Same API**: Message-based interface identical to `ChannelEmulator`
- **Clean separation**: Emulation and UI are truly independent

## Testing and Validation

A test program (`/tmp/send_test.rs`) was created to verify the approach:

```rust
// Simulating the Nes structure
struct Nes {
    ppu: Rc<RefCell<Ppu>>,
}

// This would NOT compile (Nes is not Send):
// std::thread::spawn(move || {
//     let nes = Nes { ppu: Rc::new(RefCell::new(Ppu)) };
// });

// But this DOES work (create Nes inside the thread):
std::thread::spawn(|| {
    let nes = Nes { ppu: Rc::new(RefCell::new(Ppu)) };
    // ✓ Nes never crossed thread boundary!
});
```

## Next Steps

To fully integrate this into the frontend:

1. ✅ **Create `ThreadedEmulator`** - Implemented in `frontend/src/threaded_emu.rs`
2. 🔄 **Add feature flag or config option** - Allow users to choose threaded vs non-threaded
3. 🔄 **Update `EguiApp`** - Support both `ChannelEmulator` and `ThreadedEmulator`
4. 🔄 **Performance testing** - Measure actual performance improvements
5. 🔄 **Documentation** - Update user docs about threading option

## Conclusion

The key insight is that **Send is only required when moving values between threads**. By creating `Nes` on the background thread rather than moving it there, we avoid the need for `Send` entirely. This allows us to keep the fast `Rc<RefCell<>>` pattern while still achieving true multi-threaded execution with a responsive UI.

The previous attempt failed because it tried to use `Arc<Mutex<>>` for thread safety, which added significant overhead. This approach avoids that overhead entirely while still providing the benefits of multi-threading.
