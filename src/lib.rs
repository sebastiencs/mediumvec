//! A vector that is indexed by `u32` instead of `usize`.

// Copyright 2017 Matt Brubeck.
//
// Licensed under the Apache License, Version 2.0 <http://www.apache.org/licenses/LICENSE-2.0> or
// the MIT license <http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;

pub mod vec32;
pub mod vec8;

pub use vec32::Vec32;
pub use vec8::Vec8;
