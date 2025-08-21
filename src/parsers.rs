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
