//! Memory initialization utilities for CLI.
//!
//! This module handles parsing and applying memory initialization commands
//! from CLI arguments or configuration files.
//!
//! # Supported Formats
//!
//! ## Command Line Format
//! - Single value: `0x0050=0xFF`
//! - Multiple values: `0x0050=0x01,0x02,0x03,0x04`
//!
//! ## File Formats (--init-file)
//! - JSON
//! - TOML
//! - Binary

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use monsoon_core::emulation::nes::Nes;

/// Represents a memory initialization operation
#[derive(Debug, Clone)]
pub struct MemoryInit {
    /// Starting address
    pub address: u16,
    /// Values to write starting at address
    pub values: Vec<u8>,
}

impl MemoryInit {
    /// Parse a memory init string in format `ADDR=VALUE` or `ADDR=V1,V2,...`
    ///
    /// # Examples
    ///
    /// ```
    /// use monsoon_cli::cli::memory_init::MemoryInit;
    ///
    /// let init = MemoryInit::parse("0x0050=0xFF").unwrap();
    /// assert_eq!(init.address, 0x0050);
    /// assert_eq!(init.values, vec![0xFF]);
    ///
    /// let init = MemoryInit::parse("0x6000=0x01,0x02,0x03").unwrap();
    /// assert_eq!(init.address, 0x6000);
    /// assert_eq!(init.values, vec![0x01, 0x02, 0x03]);
    /// ```
    pub fn parse(s: &str) -> Result<Self, String> {
        let (addr_str, values_str) = s.split_once('=').ok_or_else(|| {
            format!(
                "Invalid memory init format '{}'. Expected ADDR=VALUE or ADDR=V1,V2,...",
                s
            )
        })?;

        let address = parse_hex_u16(addr_str.trim())?;
        let values = parse_values(values_str.trim())?;

        if values.is_empty() {
            return Err(format!("Memory init '{}' has no values", s));
        }

        Ok(Self {
            address,
            values,
        })
    }
}

/// Parse a hexadecimal u16 value
fn parse_hex_u16(s: &str) -> Result<u16, String> {
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    u16::from_str_radix(s, 16).map_err(|e| format!("Invalid hex address '{}': {}", s, e))
}

/// Parse a comma-separated list of hex u8 values
fn parse_values(s: &str) -> Result<Vec<u8>, String> {
    s.split(',')
        .map(|v| {
            let v = v.trim();
            let v = v
                .strip_prefix("0x")
                .or_else(|| v.strip_prefix("0X"))
                .unwrap_or(v);
            u8::from_str_radix(v, 16).map_err(|e| format!("Invalid hex value '{}': {}", v, e))
        })
        .collect()
}

/// Memory initialization configuration loaded from a file
#[derive(Debug, Clone, Default)]
pub struct MemoryInitConfig {
    /// CPU memory initializations
    pub cpu: HashMap<u16, Vec<u8>>,
    /// PPU memory initializations
    pub ppu: HashMap<u16, Vec<u8>>,
    /// OAM memory initializations
    pub oam: HashMap<u16, Vec<u8>>,
}

impl MemoryInitConfig {
    /// Load memory init configuration from a file.
    ///
    /// Supports JSON, TOML, and binary formats.
    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match extension.as_str() {
            "json" => Self::load_json(path),
            "toml" => Self::load_toml(path),
            "bin" | "binary" => Self::load_binary(path),
            _ => Err(format!(
                "Unsupported init file format '{}'. Use .json, .toml, or .bin",
                extension
            )),
        }
    }

    /// Load from JSON file
    fn load_json(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read init file '{}': {}", path.display(), e))?;

        let json: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse JSON init file: {}", e))?;

        Self::from_json_value(&json)
    }

    /// Load from TOML file
    fn load_toml(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read init file '{}': {}", path.display(), e))?;

        let toml: toml::Value = content
            .parse()
            .map_err(|e| format!("Failed to parse TOML init file: {}", e))?;

        Self::from_toml_value(&toml)
    }

    /// Load from binary file (raw bytes for CPU memory starting at 0x0000)
    fn load_binary(path: &Path) -> Result<Self, String> {
        let mut file = std::fs::File::open(path).map_err(|e| {
            format!(
                "Failed to open binary init file '{}': {}",
                path.display(),
                e
            )
        })?;

        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|e| format!("Failed to read binary init file: {}", e))?;

        let mut config = Self::default();
        config.cpu.insert(0x0000, data);
        Ok(config)
    }

    /// Parse from JSON value
    fn from_json_value(json: &serde_json::Value) -> Result<Self, String> {
        let mut config = Self::default();

        if let Some(cpu) = json.get("cpu").and_then(|v| v.as_object()) {
            for (addr_str, values) in cpu {
                let addr = parse_hex_u16(addr_str)?;
                let vals = parse_json_array(values)?;
                config.cpu.insert(addr, vals);
            }
        }

        if let Some(ppu) = json.get("ppu").and_then(|v| v.as_object()) {
            for (addr_str, values) in ppu {
                let addr = parse_hex_u16(addr_str)?;
                let vals = parse_json_array(values)?;
                config.ppu.insert(addr, vals);
            }
        }

        if let Some(oam) = json.get("oam").and_then(|v| v.as_object()) {
            for (addr_str, values) in oam {
                let addr = parse_hex_u16(addr_str)?;
                let vals = parse_json_array(values)?;
                config.oam.insert(addr, vals);
            }
        }

        Ok(config)
    }

    /// Parse from TOML value
    fn from_toml_value(toml: &toml::Value) -> Result<Self, String> {
        let mut config = Self::default();

        if let Some(cpu) = toml.get("cpu").and_then(|v| v.as_table()) {
            for (addr_str, values) in cpu {
                let addr = parse_hex_u16(addr_str)?;
                let vals = parse_toml_array(values)?;
                config.cpu.insert(addr, vals);
            }
        }

        if let Some(ppu) = toml.get("ppu").and_then(|v| v.as_table()) {
            for (addr_str, values) in ppu {
                let addr = parse_hex_u16(addr_str)?;
                let vals = parse_toml_array(values)?;
                config.ppu.insert(addr, vals);
            }
        }

        if let Some(oam) = toml.get("oam").and_then(|v| v.as_table()) {
            for (addr_str, values) in oam {
                let addr = parse_hex_u16(addr_str)?;
                let vals = parse_toml_array(values)?;
                config.oam.insert(addr, vals);
            }
        }

        Ok(config)
    }
}

/// Parse a JSON array of numbers to u8 values
fn parse_json_array(value: &serde_json::Value) -> Result<Vec<u8>, String> {
    match value {
        serde_json::Value::Array(arr) => arr
            .iter()
            .map(|v| match v {
                serde_json::Value::Number(n) => n
                    .as_u64()
                    .filter(|&n| n <= 255)
                    .map(|n| n as u8)
                    .ok_or_else(|| format!("Value {} out of range for u8", n)),
                serde_json::Value::String(s) => {
                    let s = s
                        .strip_prefix("0x")
                        .or_else(|| s.strip_prefix("0X"))
                        .unwrap_or(s);
                    u8::from_str_radix(s, 16)
                        .map_err(|e| format!("Invalid hex value '{}': {}", s, e))
                }
                _ => Err("Expected number or hex string".to_string()),
            })
            .collect(),
        serde_json::Value::Number(n) => {
            let val = n
                .as_u64()
                .filter(|&n| n <= 255)
                .map(|n| n as u8)
                .ok_or_else(|| format!("Value {} out of range for u8", n))?;
            Ok(vec![val])
        }
        _ => Err("Expected array or number".to_string()),
    }
}

/// Parse a TOML array of numbers to u8 values
fn parse_toml_array(value: &toml::Value) -> Result<Vec<u8>, String> {
    match value {
        toml::Value::Array(arr) => arr
            .iter()
            .map(|v| match v {
                toml::Value::Integer(n) => {
                    if *n >= 0 && *n <= 255 {
                        Ok(*n as u8)
                    } else {
                        Err(format!("Value {} out of range for u8", n))
                    }
                }
                toml::Value::String(s) => {
                    let s = s
                        .strip_prefix("0x")
                        .or_else(|| s.strip_prefix("0X"))
                        .unwrap_or(s);
                    u8::from_str_radix(s, 16)
                        .map_err(|e| format!("Invalid hex value '{}': {}", s, e))
                }
                _ => Err("Expected integer or hex string".to_string()),
            })
            .collect(),
        toml::Value::Integer(n) => {
            if *n >= 0 && *n <= 255 {
                Ok(vec![*n as u8])
            } else {
                Err(format!("Value {} out of range for u8", n))
            }
        }
        _ => Err("Expected array or integer".to_string()),
    }
}

/// Apply memory initialization operations to the emulator
pub fn apply_memory_init(
    emu: &mut Nes,
    cpu_inits: &[MemoryInit],
    ppu_inits: &[MemoryInit],
    oam_inits: &[MemoryInit],
) {
    // Apply CPU memory initializations
    for init in cpu_inits {
        for (offset, &value) in init.values.iter().enumerate() {
            let addr = init.address.wrapping_add(offset as u16);
            emu.cpu_mem_write(addr, value);
        }
    }

    // Apply PPU memory initializations
    for init in ppu_inits {
        for (offset, &value) in init.values.iter().enumerate() {
            let addr = init.address.wrapping_add(offset as u16);
            emu.ppu_mem_write(addr, value);
        }
    }

    // Apply OAM memory initializations
    for init in oam_inits {
        for (offset, &value) in init.values.iter().enumerate() {
            // OAM is limited to 256 bytes (addresses 0x00-0xFF)
            let addr = (init.address.wrapping_add(offset as u16)) & 0xFF;
            emu.oam_write(addr, value);
        }
    }
}

/// Apply memory initialization from config file
pub fn apply_memory_init_config(emu: &mut Nes, config: &MemoryInitConfig) {
    // Apply CPU memory initializations
    for (&addr, values) in &config.cpu {
        for (offset, &value) in values.iter().enumerate() {
            let target_addr = addr.wrapping_add(offset as u16);
            emu.cpu_mem_write(target_addr, value);
        }
    }

    // Apply PPU memory initializations
    for (&addr, values) in &config.ppu {
        for (offset, &value) in values.iter().enumerate() {
            let target_addr = addr.wrapping_add(offset as u16);
            emu.ppu_mem_write(target_addr, value);
        }
    }

    // Apply OAM memory initializations
    for (&addr, values) in &config.oam {
        for (offset, &value) in values.iter().enumerate() {
            let target_addr = (addr.wrapping_add(offset as u16)) & 0xFF;
            emu.oam_write(target_addr, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_value() {
        let init = MemoryInit::parse("0x0050=0xFF").unwrap();
        assert_eq!(init.address, 0x0050);
        assert_eq!(init.values, vec![0xFF]);
    }

    #[test]
    fn test_parse_multiple_values() {
        let init = MemoryInit::parse("0x6000=0x01,0x02,0x03,0x04").unwrap();
        assert_eq!(init.address, 0x6000);
        assert_eq!(init.values, vec![0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn test_parse_without_0x_prefix() {
        let init = MemoryInit::parse("6000=FF").unwrap();
        assert_eq!(init.address, 0x6000);
        assert_eq!(init.values, vec![0xFF]);
    }

    #[test]
    fn test_parse_with_spaces() {
        let init = MemoryInit::parse("0x0050 = 0x01, 0x02, 0x03").unwrap();
        assert_eq!(init.address, 0x0050);
        assert_eq!(init.values, vec![0x01, 0x02, 0x03]);
    }

    #[test]
    fn test_parse_invalid_format() {
        assert!(MemoryInit::parse("0x0050").is_err());
        assert!(MemoryInit::parse("invalid").is_err());
    }

    #[test]
    fn test_parse_empty_values() {
        assert!(MemoryInit::parse("0x0050=").is_err());
    }
}
