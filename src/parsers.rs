// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause

use core::str;
use std::{fs::File, io::Read, str::FromStr};

use color_print::cformat;
use num_traits::Num;

pub fn parse_number<T: Num + FromStr>(s: &str) -> Result<T, String> {
    match s.strip_prefix("0x") {
        Some(stripped) => {
            T::from_str_radix(stripped, 16).or(Err(cformat!("hex number '<y>{s}</>' is invalid or too large")))
        }
        None => s
            .parse()
            .or(Err(cformat!("number '<y>{s}</>' is invalid or too large!"))),
    }
}

#[allow(dead_code, reason = "this function is used in main function by clap")]
pub fn parse_key_sel(s: &str) -> Result<u32, String> {
    match s.to_ascii_uppercase().as_str() {
        "OPTMK" => Ok(0),
        "ZMK" => Ok(2),
        "CMK" => Ok(3),
        _ => {
            let key_sel = parse_number::<u32>(s)?;
            if key_sel <= 3 {
                Ok(key_sel)
            } else {
                Err(format!(
                    "key selector '{s}' is invalid: expected 0, 1, 2, 3, OPTMK, ZMK, or CMK"
                ))
            }
        }
    }
}

/// Parses a hexadecimal number with an optional `0x` prefix, matching blhost's `INT(base=16)`
/// used e.g. for `efuse-program-once`'s `DATA` argument.
#[allow(dead_code, reason = "this function is used in main function by clap")]
pub fn parse_hex_number<T: Num>(s: &str) -> Result<T, String> {
    let digits = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(s);
    T::from_str_radix(digits, 16).or(Err(cformat!("hex number '<y>{s}</>' is invalid or too large")))
}

/// Parses a byte count restricted to 4 or 8, matching blhost's `click.Choice(["4", "8"])` for
/// `flash-program-once`/`flash-read-once`'s `BYTE_COUNT` argument.
#[allow(dead_code, reason = "this function is used in main function by clap")]
pub fn parse_once_byte_count(s: &str) -> Result<u32, String> {
    let count = parse_number::<u32>(s)?;
    if count == 4 || count == 8 {
        Ok(count)
    } else {
        Err(cformat!("count '<y>{s}</>' is invalid: only 4 or 8 are supported"))
    }
}

pub fn parse_file(s: &str, limit: Option<usize>) -> Result<Box<[u8]>, String> {
    let mut file = File::open(s).map_err(|err| err.to_string())?;
    Ok(if let Some(limit) = limit {
        let mut buf = vec![0u8; limit];
        file.read_exact(&mut buf).map_err(|err| err.to_string())?;
        buf
    } else {
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).map_err(|err| err.to_string())?;
        buf
    }
    .into_boxed_slice())
}

#[allow(dead_code, reason = "this function is used in main function by clap")]
pub fn parse_hex_values(s: &str) -> Result<Box<[u8]>, String> {
    if s.starts_with("{{") {
        let s = s.trim_matches(|c| c == '{' || c == '}').replace(' ', "");
        if s.len() % 2 != 0 {
            return Err("length of the input is not odd".to_owned());
        }
        s.as_bytes()
            .chunks(2)
            .map(|ch| {
                let converted = str::from_utf8(ch).unwrap();
                u8::from_str_radix(converted, 16).or(Err(cformat!("invalid byte: '<y>{converted}'")))
            })
            .collect()
    } else {
        match s.find(',') {
            Some(index) => parse_file(&s[..index], Some(parse_number(&s[index + 1..])?)),
            None => parse_file(s, None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_key_sel;

    #[test]
    fn test_parse_key_sel_names_and_numbers() {
        for (value, expected) in [
            ("0", 0),
            ("1", 1),
            ("2", 2),
            ("3", 3),
            ("OPTMK", 0),
            ("zmk", 2),
            ("CMK", 3),
        ] {
            assert_eq!(parse_key_sel(value).unwrap(), expected);
        }
    }

    #[test]
    fn test_parse_key_sel_rejects_unsupported_values() {
        assert!(parse_key_sel("4").is_err());
        assert!(parse_key_sel("INVALID").is_err());
    }
}
