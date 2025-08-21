// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
use std::fmt::{Debug, Display};

use log::error;
use num_traits::ToPrimitive;
use number_prefix::NumberPrefix;

/// Implements Display to print a number as bytes with IEC binary prefix.
///
/// Prints with one decimal place if there is a prefix, without prefix there are no decimal
/// places.
pub struct BinaryBytesOne<T>(pub T)
where
    T: ToPrimitive;

impl<T> Display for BinaryBytesOne<T>
where
    T: ToPrimitive + Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let num = self.0.to_f64().unwrap_or_else(|| {
            error!(
                "could not convert number/type {:?} to f64 to display it in bytes, using f64::NAN instead",
                self.0
            );
            f64::NAN
        });
        match NumberPrefix::binary(num) {
            NumberPrefix::Standalone(number) => write!(f, "{number:.0} B"),
            NumberPrefix::Prefixed(prefix, number) => write!(f, "{number:.1} {prefix}B"),
        }
    }
}

/// Implements Display, displays `bool` as "ON" if `true` and "OFF" if `false`.
pub struct OnOffBool(pub bool);

impl Display for OnOffBool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0 { "ON" } else { "OFF" })
    }
}
