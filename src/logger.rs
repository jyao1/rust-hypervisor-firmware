// Copyright © 2019 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// Inspired by https://github.com/phil-opp/blog_os/blob/post-03/src/vga_buffer.rs
// from Philipp Oppermann

use core::fmt;
use lazy_static::lazy_static;
use spin::Mutex;

use cpuio::Port;

pub const LOG_LEVEL_VERBOSE : usize = 1000;
pub const LOG_LEVEL_INFO    : usize = 100;
pub const LOG_LEVEL_WARN    : usize = 10;
pub const LOG_LEVEL_ERROR   : usize = 1;
pub const LOG_LEVEL_NONE    : usize = 0;

pub const LOG_MASK_COMMON       : u64 = 0x1;
// Core - Boot Service (BIT1 ~ BIT15)
pub const LOG_MASK_PROTOCOL     : u64 = 0x2;
pub const LOG_MASK_MEMORY       : u64 = 0x4;
pub const LOG_MASK_EVENT        : u64 = 0x8;
pub const LOG_MASK_IMAGE        : u64 = 0x10;
// Core - Runtime Service (BIT16 ~ BIT 23)
pub const LOG_MASK_VARIABLE     : u64 = 0x10000;
// Core - Console (BIT24 ~ BIT 31)
pub const LOG_MASK_CONOUT       : u64 = 0x1000000;
pub const LOG_MASK_CONIN        : u64 = 0x2000000;
// Protocol - (BIT32 ~ BIT63)
pub const LOG_MASK_BLOCK_IO     : u64 = 0x100000000;
pub const LOG_MASK_FILE_SYSTEM  : u64 = 0x200000000;
// All
pub const LOG_MASK_ALL          : u64 = 0xFFFFFFFFFFFFFFFF;

lazy_static! {
    static ref LOGGER: Mutex<Logger> = Mutex::new(Logger {
        port: unsafe { Port::new(0x3f8) },
        level: LOG_LEVEL_VERBOSE,
        mask: LOG_MASK_ALL,
    });
}

struct Logger {
    port: Port<u8>,
    level: usize,
    mask: u64,
}

impl Logger {
    pub fn write_byte(&mut self, byte: u8) {
        if byte == '\n' as u8 {
          self.port.write('\r' as u8)
        }
        self.port.write(byte)
    }

    pub fn write_string(&mut self, s: &str) {
        for c in s.chars() {
            self.write_byte(c as u8);
        }
    }

    pub fn get_level(&mut self) -> usize {
        self.level
    }
    pub fn set_level(&mut self, level: usize) {
        self.level = level;
    }

    pub fn get_mask(&mut self) -> u64 {
        self.mask
    }
    pub fn set_mask(&mut self, mask: u64) {
        self.mask = mask;
    }
}

impl fmt::Write for Logger {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => ($crate::logger::_log_ex(crate::logger::LOG_LEVEL_VERBOSE, crate::logger::LOG_MASK_COMMON, format_args!($($arg)*)));
    //($($arg:tt)*) => ($crate::logger::_log_ex(crate::logger::LOG_LEVEL_VERBOSE, 0, format_args!($($arg)*)));
}

#[macro_export]
macro_rules! log_ex {
    ($level:expr, $mask:expr, $($arg:tt)*) => ($crate::logger::_log_ex($level, $mask, format_args!($($arg)*)));
}

#[macro_export]
macro_rules! log_always {
    ($($arg:tt)*) => ($crate::logger::_log(format_args!($($arg)*)));
}

#[cfg(not(test))]
pub fn _log(args: fmt::Arguments) {
    use core::fmt::Write;
    LOGGER.lock().write_fmt(args).unwrap();
}

#[cfg(not(test))]
pub fn _log_ex(level: usize, mask: u64, args: fmt::Arguments) {
    if level > LOGGER.lock().get_level() {
      return 
    }
    if (mask & LOGGER.lock().get_mask()) == 0 {
      return 
    }
    _log (args);
}

#[cfg(test)]
pub fn _log(args: fmt::Arguments) {
    use std::io::{self, Write};
    write!(&mut std::io::stdout(), "{}", args).expect("stdout logging failed");
}
