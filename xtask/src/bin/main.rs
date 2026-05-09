use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use quick_xml::encoding::EncodingError;
use quick_xml::events::attributes::AttrError;
use quick_xml::events::Event;
use quick_xml::Reader;

fn main() {
    println!("Running xtasks");

    let mut args = std::env::args();
    args.next();

    match args.next().as_deref() {
        Some("merge") => {
            let merge_res = merge_db();
            if let Err(err) = merge_res {
                println!("{err}")
            }
        }

        Some(cmd) => {
            eprintln!("Unknown command: {cmd}");
        }

        None => {
            eprintln!("No command provided");
        }
    }
}

fn merge_db() -> Result<(), MergeError> {
    println!("Merging Rom DB files");
    let headered_file = include_str!("../../assets/no-intro-db.xml");
    let headerless_file = include_str!("../../assets/no-intro-db-headerless.xml");
    let alt_name_file = include_str!("../../assets/no-intro-db-alt-names.xml");

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

    let mut games = HashMap::new();

    loop {
        match headered_reader.read_event_into(&mut buf)? {
            Event::Start(e) => match e.name().as_ref() {
                b"version" => {
                    in_version = true;
                }

                b"game" => {
                    for attr in e.attributes() {
                        let attr = attr?;

                        if attr.key.as_ref() == b"name" {
                            current_game_name = Some(attr.unescape_value()?.into_owned());
                        }
                    }
                }

                b"rom" => {
                    let mut sha256 = String::new();
                    let mut header = None;

                    for attr in e.attributes() {
                        let attr = attr?;

                        if attr.key.as_ref() == b"sha256" {
                            sha256 = attr.unescape_value()?.into_owned();
                        }

                        if attr.key.as_ref() == b"header" {
                            header =Some(attr.unescape_value()?.into_owned())
                        }
                    }

                    if let Some(game_name) = &current_game_name {
                        let bytes = hex::decode(sha256)?;

                        let hash: [u8; 32] = bytes.try_into()?;

                        games.insert(
                            hash,
                            TempEntry {
                                name: game_name.clone(),
                                orig_name: None,
                                headered_sha256: Some(hash),
                                unheadered_sha256: None,
                                header
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
            Event::Start(e) => match e.name().as_ref() {
                b"game" => {
                    for attr in e.attributes() {
                        let attr = attr?;

                        if attr.key.as_ref() == b"name" {
                            current_game_name = attr.unescape_value()?.into_owned();
                        }
                    }
                }

                b"rom" => {
                    let mut sha256 = String::new();

                    for attr in e.attributes() {
                        let attr = attr?;

                        if attr.key.as_ref() == b"sha256" {
                            sha256 = attr.unescape_value()?.into_owned();
                        }
                    }

                    let bytes = hex::decode(sha256)?;

                    let hash: [u8; 32] = bytes.try_into()?;

                    let game = games
                        .iter_mut()
                        .find(|(_, game)| game.name == current_game_name);

                    if let Some((_, game)) = game {
                        game.unheadered_sha256 = Some(hash);
                    } else {
                        return Err(MergeError::GameListMismatch(current_game_name));
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
            Event::Start(e) => match e.name().as_ref() {
                b"game" => {
                    for attr in e.attributes() {
                        let attr = attr?;

                        if attr.key.as_ref() == b"name" {
                            curr_name = Some(attr.unescape_value()?.into_owned());
                        }
                    }
                }

                b"rom" => {
                    let mut sha256 = String::new();

                    for attr in e.attributes() {
                        let attr = attr?;

                        if attr.key.as_ref() == b"sha256" {
                            sha256 = attr.unescape_value()?.into_owned();
                        }
                    }

                    let bytes = hex::decode(sha256.clone())?;
                    let hash: [u8; 32] = bytes.try_into()?;

                    let game = games.get_mut(&hash);

                    if let Some(game) = game {
                        game.orig_name = curr_name.clone()
                    } else {
                        return Err(MergeError::GameListMismatch(sha256));
                    }
                }

                _ => {}
            },

            Event::Eof => break,
            _ => {}
        }

        buf.clear();
    }

    println!("{games:#?}");
    println!("version: {version:?}");

    Ok(())
}

#[derive(Debug)]
struct TempEntry {
    name: String,
    orig_name: Option<String>,
    headered_sha256: Option<[u8; 32]>,
    unheadered_sha256: Option<[u8; 32]>,
    header: Option<String>
}

#[derive(Debug, Clone)]
enum MergeError {
    XmlParse(quick_xml::Error),
    AttributeParse(AttrError),
    XmlEncoding(EncodingError),
    ChecksumDecode(hex::FromHexError),
    ChecksumToBytes(Vec<u8>),
    GameListMismatch(String),
}

impl Display for MergeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Error while Merging databases")?;
        match self {
            MergeError::XmlParse(e) => f.write_str(format!("{e}").as_str()),
            MergeError::AttributeParse(e) => f.write_str(format!("{e}").as_str()),
            MergeError::XmlEncoding(e) => f.write_str(format!("{e}").as_str()),
            MergeError::ChecksumDecode(e) => f.write_str(format!("{e}").as_str()),
            MergeError::ChecksumToBytes(e) => f.write_str(format!("{e:?}").as_str()),
            MergeError::GameListMismatch(e) => f.write_str(e.as_str()),
        }
    }
}

impl From<quick_xml::Error> for MergeError {
    fn from(value: quick_xml::Error) -> Self { MergeError::XmlParse(value) }
}

impl From<hex::FromHexError> for MergeError {
    fn from(value: hex::FromHexError) -> Self { MergeError::ChecksumDecode(value) }
}

impl From<AttrError> for MergeError {
    fn from(value: AttrError) -> Self { MergeError::AttributeParse(value) }
}

impl From<EncodingError> for MergeError {
    fn from(value: EncodingError) -> Self { MergeError::XmlEncoding(value) }
}

impl From<Vec<u8>> for MergeError {
    fn from(value: Vec<u8>) -> Self { MergeError::ChecksumToBytes(value) }
}
