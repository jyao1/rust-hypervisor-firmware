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
use core::ffi::c_void;

use r_efi::protocols::simple_text_output::Mode as SimpleTextOutputMode;

use crate::efi::STDOUT_MODE;

const EFI_BLACK                 : u8 = 0x00;
const EFI_BLUE                  : u8 = 0x01;
const EFI_GREEN                 : u8 = 0x02;
const EFI_CYAN                  : u8 = (EFI_BLUE | EFI_GREEN);
const EFI_RED                   : u8 = 0x04;
const EFI_MAGENTA               : u8 = (EFI_BLUE | EFI_RED);
const EFI_BROWN                 : u8 = (EFI_GREEN | EFI_RED);
const EFI_LIGHTGRAY             : u8 = (EFI_BLUE | EFI_GREEN | EFI_RED);
const EFI_BRIGHT                : u8 = 0x08;

const ESC                       : u8 = 0x1B;
const BRIGHT_CONTROL_OFFSET     : usize = 2;
const FOREGROUND_CONTROL_OFFSET : usize = 6;
const BACKGROUND_CONTROL_OFFSET : usize = 11;
const ROW_OFFSET                : usize = 2;
const COLUMN_OFFSET             : usize = 5;

const SET_MODE_STRING_SIZE            : usize = 6;
const SET_ATTRIBUTE_STRING_SIZE       : usize = 15;
const CLEAR_SCREEN_STRING_SIZE        : usize = 5;
const SET_CURSOR_POSITION_STRING_SIZE : usize = 9;
const CURSOR_FORWARD_STRING_SIZE      : usize = 6;
const CURSOR_BACKWARD_STRING_SIZE     : usize = 6;

pub struct ConOut {
    port: Port<u8>,
    mode_ptr: usize,
    set_mode_string: [u8; SET_MODE_STRING_SIZE],
    set_attribute_string: [u8; SET_ATTRIBUTE_STRING_SIZE],
    clear_screen_string: [u8; CLEAR_SCREEN_STRING_SIZE],
    set_cursor_position_string: [u8; SET_CURSOR_POSITION_STRING_SIZE],
    cursor_forward_string: [u8; CURSOR_FORWARD_STRING_SIZE],
    cursor_backward_string: [u8; CURSOR_BACKWARD_STRING_SIZE],
}

impl ConOut {
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

    pub fn set_cursor_position(&mut self, column: usize, row: usize) {
        self.set_cursor_position_string[ROW_OFFSET + 0]    = ('0' as usize + ((row + 1) / 10)) as u8;
        self.set_cursor_position_string[ROW_OFFSET + 1]    = ('0' as usize + ((row + 1) % 10)) as u8;
        self.set_cursor_position_string[COLUMN_OFFSET + 0] = ('0' as usize + ((column + 1) / 10)) as u8;
        self.set_cursor_position_string[COLUMN_OFFSET + 1] = ('0' as usize + ((column + 1) % 10)) as u8;

        for i in 0 .. SET_CURSOR_POSITION_STRING_SIZE {
            let c = self.set_cursor_position_string[i];
            //self.write_byte(c);
        }

        if column == 0 && row == 0 {
            self.write_byte('\r' as u8);
        }
    }

    pub fn set_attribute(&mut self, attribute: usize) {
        if (attribute | 0x7f) != 0x7f {
          return ;
        }

        let mut foreground_control : usize = 0;
        match (attribute & 0x7) as u8 {
          EFI_BLACK => {foreground_control = 30},
          EFI_BLUE => {foreground_control = 34},
          EFI_GREEN => {foreground_control = 32},
          EFI_CYAN => {foreground_control = 36},
          EFI_RED => {foreground_control = 31},
          EFI_MAGENTA => {foreground_control = 35},
          EFI_BROWN => {foreground_control = 33},
          EFI_LIGHTGRAY => {foreground_control = 37},
          _ => {foreground_control = 37},
        }
        
        let mut bright_control : usize = (attribute >> 3) & 1;

        let mut background_control : usize = 0;
        match ((attribute >> 4) & 0x7) as u8 {
          EFI_BLACK => {background_control = 40},
          EFI_BLUE => {background_control = 44},
          EFI_GREEN => {background_control = 42},
          EFI_CYAN => {background_control = 46},
          EFI_RED => {background_control = 41},
          EFI_MAGENTA => {background_control = 45},
          EFI_BROWN => {background_control = 43},
          EFI_LIGHTGRAY => {background_control = 47},
          _ => {background_control = 47},
        }

        self.set_attribute_string[BRIGHT_CONTROL_OFFSET]         = ('0' as usize + bright_control) as u8;
        self.set_attribute_string[FOREGROUND_CONTROL_OFFSET + 0] = ('0' as usize + (foreground_control / 10)) as u8;
        self.set_attribute_string[FOREGROUND_CONTROL_OFFSET + 1] = ('0' as usize + (foreground_control % 10)) as u8;
        self.set_attribute_string[BACKGROUND_CONTROL_OFFSET + 0] = ('0' as usize + (background_control / 10)) as u8;
        self.set_attribute_string[BACKGROUND_CONTROL_OFFSET + 1] = ('0' as usize + (background_control % 10)) as u8;

        for i in 0 .. SET_ATTRIBUTE_STRING_SIZE {
            let c = self.set_attribute_string[i];
            //self.write_byte(c);
        }
    }

    pub fn clear_screen(&mut self) {
        for i in 0 .. CLEAR_SCREEN_STRING_SIZE {
            let c = self.clear_screen_string[i];
            //self.write_byte(c);
        }
    }

    pub fn new() -> ConOut {
        ConOut {
            port: unsafe { Port::new(0x3f8) },
            mode_ptr: unsafe { &mut STDOUT_MODE as *mut SimpleTextOutputMode as usize },
            set_mode_string:            [ ESC, '[' as u8, '=' as u8, '3' as u8, 'h' as u8, 0 ],
            set_attribute_string:       [ ESC, '[' as u8, '0' as u8, 'm' as u8, ESC, '[' as u8, '4' as u8, '0' as u8, 'm' as u8, ESC, '[' as u8, '4' as u8, '0' as u8, 'm' as u8, 0 ],
            clear_screen_string:        [ ESC, '[' as u8, '2' as u8, 'J' as u8, 0 ],
            set_cursor_position_string: [ ESC, '[' as u8, '0' as u8, '0' as u8, ';' as u8, '0' as u8, '0' as u8, 'H' as u8, 0 ],
            cursor_forward_string:      [ ESC, '[' as u8, '0' as u8, '0' as u8, 'C' as u8, 0 ],
            cursor_backward_string:     [ ESC, '[' as u8, '0' as u8, '0' as u8, 'D' as u8, 0 ],
        }
    }
}

impl fmt::Write for ConOut {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}
