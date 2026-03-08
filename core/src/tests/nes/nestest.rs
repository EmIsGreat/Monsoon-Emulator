use std::fs::File;
use std::io;
use std::io::BufRead;

use crate::emulation::nes::Nes;

#[test]
fn nestest() {
    let mut emu = Nes::default();
    emu.enable_trace();
    emu.load_rom(&String::from("./tests/nes-test-roms/nestest_headless.nes"));
    emu.power();
    emu.run().expect("Error running test");

    let log = emu
        .trace_log
        .unwrap()
        .log
        .lines()
        .map(|s| s.to_string())
        .take(8980)
        .collect::<Vec<String>>();

    let file = File::open("./tests/outputs-compare/nestest_headless_good.log")
        .expect("Error running test");
    let lines = io::BufReader::new(file)
        .lines()
        .take(8980)
        .collect::<Result<Vec<_>, _>>()
        .expect("Error running test");

    assert_eq!(log, lines);
}
