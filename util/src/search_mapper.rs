use std::collections::HashMap;
use std::fmt::Display;
use std::fs;
use std::hash::Hash;
use std::path::PathBuf;
use std::str::FromStr;

use monsoon_core::emulation::rom::{RomFile, RomMapper};
use monsoon_core::rom_db::RomDb;
use strum::IntoEnumIterator;
use walkdir::WalkDir;

pub fn search_for_mapper(mapper: u16, use_local: bool) {
    print_with_predicate(
        |f| <RomMapper as Into<u16>>::into(f.mapper) == mapper,
        &vec![].into_boxed_slice(),
        use_local,
    );
}

fn get_roms(use_local: bool) -> Vec<RomFile> {
    let mut roms: Vec<RomFile> = if use_local {
        let path = PathBuf::from_str("/home/emily/roms/nes/").unwrap();

        let roms: Vec<_> = WalkDir::new(path)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "nes"))
            .collect();

        roms.iter()
            .map(|f| {
                RomFile::try_from((
                    &mut fs::read(f.path()).unwrap()[..],
                    &f.path().file_name().unwrap().to_string_lossy().to_string(),
                    false,
                    None,
                ))
                .unwrap()
            })
            .collect()
    } else {
        let db = RomDb::default();
        db.get_all_entries()
            .iter()
            .copied()
            .cloned()
            .filter_map(|f| {
                f.header
                    .clone()
                    .and_then(|h| RomFile::get_for_header(&h, &f.name).ok())
            })
            .collect()
    };
    roms.sort_by_key(|f| f.name.clone().unwrap());
    roms
}

pub fn search_for_submapper_mapper(mapper: u16, sub: u8, use_local: bool) {
    print_with_predicate(
        |f| <RomMapper as Into<u16>>::into(f.mapper) == mapper && f.submapper_number == sub,
        &vec![].into_boxed_slice(),
        use_local,
    );
}

type RomTransformer = dyn Fn(&RomFile) -> String;

/// # Panics
/// When Rom is missing name
pub fn print_with_predicate<F>(pred: F, print: &[Box<RomTransformer>], use_local: bool)
where
    F: Fn(&RomFile) -> bool,
{
    let filtered = get_with_predicate(pred, use_local);

    for rom in &filtered {
        println!("{}", rom.name.clone().unwrap());
        for p in print {
            println!("  {}", p(rom));
        }
    }

    println!();
    println!("{} items", filtered.len());
}

pub fn get_with_predicate<F>(pred: F, use_local: bool) -> Vec<RomFile>
where
    F: Fn(&RomFile) -> bool,
{
    get_roms(use_local).into_iter().filter(pred).collect()
}

pub fn print_stats<X, G>(getter: G, use_local: bool)
where
    X: IntoEnumIterator + Eq + Hash + Display + Ord,
    G: Fn(&RomFile) -> Option<X>,
{
    let counts: HashMap<X, usize> = X::iter().map(|v| (v, 0)).collect();

    let mut vec = get_with_predicate(|_| true, use_local)
        .iter()
        .filter_map(getter)
        .fold(counts, |mut counts, item| {
            *counts.entry(item).or_insert(0) += 1;
            counts
        })
        .into_iter()
        .collect::<Vec<(X, usize)>>();

    vec.sort_by(|(v1, c1), (v2, c2)| c2.cmp(c1).then_with(|| v1.cmp(v2)));

    for (x, c) in vec {
        println!("{x}: {c}");
    }
}
