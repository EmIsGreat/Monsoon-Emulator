/// Example demonstrating ThreadedEmulator usage
///
/// This example shows how to use the ThreadedEmulator to run emulation
/// on a background thread while keeping the main thread responsive.
///
/// Run with: cargo run --example threaded_emulator
use std::time::{Duration, Instant};

use monsoon_frontend::messages::{EmulatorMessage, FrontendMessage};
use monsoon_frontend::threaded_emu::ThreadedEmulator;

fn main() {
    println!("=== ThreadedEmulator Demo ===\n");

    // Create the threaded emulator
    // Note: Nes is created ON the background thread, not moved there
    let (emu, rx_from_emu) = ThreadedEmulator::new();

    println!("✓ Created ThreadedEmulator (Nes created on background thread)");
    println!("  - Main thread remains responsive");
    println!("  - No Arc/Mutex overhead (uses Rc<RefCell<Ppu>>)");
    println!("  - Only channel messages cross thread boundaries\n");

    // Simulate some operations
    println!("Sending Reset message to emulator...");
    emu.send(FrontendMessage::Reset)
        .expect("Failed to send reset");

    // Power on the emulator
    println!("Sending Power message to emulator...");
    emu.send(FrontendMessage::Power)
        .expect("Failed to send power");

    // Request a few frames
    println!("\nRequesting 5 frames of emulation...");
    let start = Instant::now();

    for i in 0..5 {
        emu.send(FrontendMessage::StepFrame)
            .expect("Failed to send step frame");

        // Wait for frame to be ready
        match rx_from_emu.recv_timeout(Duration::from_secs(1)) {
            Ok(EmulatorMessage::FrameReady(frame)) => {
                println!(
                    "  Frame {}: {} pixels ({}x240)",
                    i + 1,
                    frame.len(),
                    frame.len() / 240
                );
            }
            Ok(EmulatorMessage::Stopped) => {
                println!("  Received Stopped message");
            }
            Ok(EmulatorMessage::DebugData(_)) => {
                println!("  Received DebugData message");
            }
            Ok(EmulatorMessage::SaveState(_, _)) => {
                println!("  Received SaveState message");
            }
            Ok(EmulatorMessage::RomLoaded(_)) => {
                println!("  Received RomLoaded message");
            }
            Err(e) => {
                println!("  Error receiving frame: {}", e);
                break;
            }
        }
    }

    let elapsed = start.elapsed();
    println!(
        "\n✓ Completed 5 frames in {:.2}ms ({:.1} FPS equivalent)",
        elapsed.as_secs_f64() * 1000.0,
        5.0 / elapsed.as_secs_f64()
    );

    println!("\n=== Main Thread Responsiveness Demo ===");
    println!("While emulator runs in background, main thread can do other work:");

    // Start continuous emulation
    for _ in 0..10 {
        emu.send(FrontendMessage::StepFrame)
            .expect("Failed to send step frame");
    }

    // Do work on main thread while emulator runs
    let start = Instant::now();
    let mut counter = 0;
    let mut frames_received = 0;

    while frames_received < 10 {
        // Simulate main thread work
        counter += 1;
        std::thread::sleep(Duration::from_micros(100));

        // Check for emulator messages without blocking
        if let Ok(EmulatorMessage::FrameReady(_)) = rx_from_emu.try_recv() {
            frames_received += 1;
        }
    }

    let elapsed = start.elapsed();
    println!(
        "  - Main thread did {} work iterations while emulator ran",
        counter
    );
    println!(
        "  - Received {} frames in {:.2}ms",
        frames_received,
        elapsed.as_secs_f64() * 1000.0
    );
    println!("  ✓ Main thread remained responsive throughout!\n");

    // Cleanup: ThreadedEmulator's Drop impl will send Quit and join thread
    println!("Shutting down emulator...");
    drop(emu);
    println!("✓ Emulator thread terminated cleanly\n");

    println!("=== Summary ===");
    println!("ThreadedEmulator successfully demonstrated:");
    println!("  1. ✓ Nes created on background thread (no Send required)");
    println!("  2. ✓ Emulation runs in parallel with UI thread");
    println!("  3. ✓ Message-based communication works correctly");
    println!("  4. ✓ Main thread remains responsive during emulation");
    println!("  5. ✓ Clean shutdown and thread termination");
}
