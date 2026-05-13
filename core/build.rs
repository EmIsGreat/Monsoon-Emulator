use std::collections::HashMap;
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use quick_xml::events::Event;
use quick_xml::events::attributes::Attribute;
use quick_xml::{Reader, XmlVersion};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RomDb {
    version: String,
    data: HashMap<[u8; 32], RomDbEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RomDbEntry {
    name: String,
    orig_name: Option<String>,
    headered_sha256: Option<[u8; 32]>,
    unheadered_sha256: Option<[u8; 32]>,
    header: Option<Vec<u8>>,
}

fn main() {
    if let Err(err) = merge_rom_db() {
        panic!("failed to regenerate rom-info-db.bin: {err}");
    }
}

fn merge_rom_db() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../Cargo.toml");
    println!("cargo:rerun-if-changed=./assets/no-intro-db.xml");
    println!("cargo:rerun-if-changed=./assets/no-intro-db-headerless.xml");
    println!("cargo:rerun-if-changed=./assets/no-intro-db-alt-names.xml");
    println!("cargo:rerun-if-changed=../Cargo.lock");

    let headered_file = include_str!("./assets/no-intro-db.xml");
    let headerless_file = include_str!("./assets/no-intro-db-headerless.xml");
    let alt_name_file = include_str!("./assets/no-intro-db-alt-names.xml");

    let mut headered_reader = Reader::from_str(headered_file);
    let mut headerless_reader = Reader::from_str(headerless_file);
    let mut alt_name_reader = Reader::from_str(alt_name_file);

    headered_reader.config_mut().trim_text(true);
    headerless_reader.config_mut().trim_text(true);
    alt_name_reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut version = None;
    let mut in_version = false;
    let mut current_game_name = None;
    let mut games: HashMap<[u8; 32], RomDbEntry> = HashMap::new();

    loop {
        match headered_reader.read_event_into(&mut buf)? {
            Event::Start(e) | Event::Empty(e) => match e.name().as_ref() {
                b"version" => in_version = true,
                b"game" => {
                    for attr in e.attributes() {
                        let attr = attr?;
                        if attr.key.as_ref() == b"name" {
                            current_game_name = Some(attr_to_string(&attr)?);
                        }
                    }
                }
                b"rom" => {
                    let mut sha256: Option<String> = None;
                    let mut header = None;
                    let mut skip_rom = false;

                    for attr in e.attributes() {
                        let attr = attr?;

                        if attr.key.as_ref() == b"status" && attr_to_string(&attr)? == "nodump" {
                            skip_rom = true;
                            break;
                        }

                        if attr.key.as_ref() == b"sha256" {
                            sha256 = Some(normalize_hex_string(&attr_to_string(&attr)?));
                        }

                        if attr.key.as_ref() == b"header" {
                            let header_string = normalize_hex_string(&attr_to_string(&attr)?);
                            header = Some(hex::decode(header_string)?);
                        }
                    }

                    if skip_rom {
                        continue;
                    }

                    if let Some(game_name) = &current_game_name {
                        let sha256 =
                            sha256.ok_or_else(|| format!("missing sha256 for {game_name}"))?;
                        let bytes = hex::decode(sha256)?;
                        let hash: [u8; 32] = bytes.try_into().map_err(|bytes: Vec<u8>| {
                            format!("expected 32-byte sha256, got {} bytes", bytes.len())
                        })?;

                        games.insert(
                            hash,
                            RomDbEntry {
                                name: game_name.clone(),
                                orig_name: None,
                                headered_sha256: Some(hash),
                                unheadered_sha256: None,
                                header,
                            },
                        );
                    }
                }
                _ => {}
            },
            Event::Text(e) if in_version => {
                version = Some(e.decode()?.into_owned());
                in_version = false;
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    let mut current_game_name = String::new();

    loop {
        match headerless_reader.read_event_into(&mut buf)? {
            Event::Start(e) | Event::Empty(e) => match e.name().as_ref() {
                b"game" => {
                    for attr in e.attributes() {
                        let attr = attr?;
                        if attr.key.as_ref() == b"name" {
                            current_game_name = attr_to_string(&attr)?;
                        }
                    }
                }
                b"rom" => {
                    let mut sha256: Option<String> = None;
                    let mut skip_rom = false;

                    for attr in e.attributes() {
                        let attr = attr?;

                        if attr.key.as_ref() == b"status" && attr_to_string(&attr)? == "nodump" {
                            skip_rom = true;
                            break;
                        }

                        if attr.key.as_ref() == b"sha256" {
                            sha256 = Some(normalize_hex_string(&attr_to_string(&attr)?));
                        }
                    }

                    if skip_rom {
                        continue;
                    }

                    let sha256 =
                        sha256.ok_or_else(|| format!("missing sha256 for {current_game_name}"))?;
                    let bytes = hex::decode(sha256)?;
                    let hash: [u8; 32] = bytes.try_into().map_err(|bytes: Vec<u8>| {
                        format!("expected 32-byte sha256, got {} bytes", bytes.len())
                    })?;

                    let game = games
                        .iter_mut()
                        .find(|(_, game)| game.name == current_game_name);

                    if let Some((_, game)) = game {
                        game.unheadered_sha256 = Some(hash);
                    } else {
                        games.insert(
                            hash,
                            RomDbEntry {
                                name: current_game_name.clone(),
                                orig_name: None,
                                headered_sha256: None,
                                unheadered_sha256: Some(hash),
                                header: None,
                            },
                        );
                    }
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    let mut curr_name = None;

    loop {
        match alt_name_reader.read_event_into(&mut buf)? {
            Event::Start(e) | Event::Empty(e) => match e.name().as_ref() {
                b"game" => {
                    for attr in e.attributes() {
                        let attr = attr?;
                        if attr.key.as_ref() == b"name" {
                            curr_name = Some(attr_to_string(&attr)?);
                        }
                    }
                }
                b"rom" => {
                    let mut sha256: Option<String> = None;
                    let mut skip_rom = false;

                    for attr in e.attributes() {
                        let attr = attr?;

                        if attr.key.as_ref() == b"status" && attr_to_string(&attr)? == "nodump" {
                            skip_rom = true;
                            break;
                        }

                        if attr.key.as_ref() == b"sha256" {
                            sha256 = Some(normalize_hex_string(&attr_to_string(&attr)?));
                        }
                    }

                    if skip_rom {
                        continue;
                    }

                    let sha256 = sha256.ok_or_else(|| {
                        format!(
                            "missing sha256 for {}",
                            curr_name.clone().unwrap_or_default()
                        )
                    })?;
                    let bytes = hex::decode(sha256.clone())?;
                    let hash: [u8; 32] = bytes.try_into().map_err(|bytes: Vec<u8>| {
                        format!("expected 32-byte sha256, got {} bytes", bytes.len())
                    })?;

                    let game = games.get_mut(&hash);

                    if let Some(game) = game {
                        game.orig_name = curr_name.clone();
                    } else {
                        games.insert(
                            hash,
                            RomDbEntry {
                                name: curr_name.clone().unwrap_or_default(),
                                orig_name: curr_name.clone(),
                                headered_sha256: Some(hash),
                                unheadered_sha256: None,
                                header: None,
                            },
                        );
                    }
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    let db = RomDb {
        version: version.unwrap_or_default(),
        data: games,
    };

    let output = postcard::to_stdvec(&db)?;
    let out_path = PathBuf::from(env::var("OUT_DIR")?).join("rom-info-db.bin");

    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(out_path)?;

    file.write_all(&output)?;
    Ok(())
}

fn attr_to_string(attr: &Attribute) -> Result<String, Box<dyn std::error::Error>> {
    Ok(attr.normalized_value(XmlVersion::Explicit1_1)?.to_string())
}

fn normalize_hex_string(value: &str) -> String {
    value.chars().filter(|c| !c.is_ascii_whitespace()).collect()
}
