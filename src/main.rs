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

#![allow(unused)]

#![feature(asm)]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]
#![cfg_attr(test, allow(unused_imports))]

#[macro_use]
mod logger;

#[macro_use]
mod common;

use core::panic::PanicInfo;

use core::ffi::c_void;
use core::mem::transmute;

use cpuio::Port;

use r_efi::efi::{
    AllocateType, MemoryType, MEMORY_WB
};

use crate::pi::hob::{
  Header, MemoryAllocation, ResourceDescription,
  RESOURCE_SYSTEM_MEMORY, HOB_TYPE_MEMORY_ALLOCATION, HOB_TYPE_RESOURCE_DESCRIPTOR, HOB_TYPE_END_OF_HOB_LIST
  };

use crate::efi::{PAGE_SIZE, ALLOCATOR};

mod block;
mod bzimage;
mod efi;
mod pi;
mod fat;
mod loader;
mod mem;
mod mmio;
mod part;
mod pci;
mod pe;
mod virtio;

#[cfg(not(test))]
#[panic_handler]
#[allow(clippy::empty_loop)]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[cfg(not(test))]
/// Reset the VM via the keyboard controller
fn i8042_reset() -> ! {
    log!("i8042_reset...\n");
    loop {
        let mut good: u8 = 0x02;
        let mut i8042_command: Port<u8> = unsafe { Port::new(0x64) };
        while good & 0x02 > 0 {
            good = i8042_command.read();
        }
        i8042_command.write(0xFE);
    }
}

#[cfg(not(test))]
/// Enable SSE2 for XMM registers (needed for EFI calling)
fn enable_sse2() {
    unsafe {
        asm!("movq %cr0, %rax");
        asm!("or $$0x2, %ax");
        asm!("movq %rax, %cr0");
        asm!("movq %cr4, %rax");
        asm!("or $$0x600, %ax");
        asm!("movq %rax, %cr4");
    }
}

#[cfg(not(test))]
// Populate allocator from E820, fixed ranges for the firmware and the loaded binary.
pub fn initialize_memory(hob: *const c_void) {

  unsafe {
    let mut hob_header : *const Header = hob as *const Header;

    loop {
      let header = transmute::<*const Header, &Header>(hob_header);
      match header.r#type {
        HOB_TYPE_RESOURCE_DESCRIPTOR => {
          let resource_hob = transmute::<*const Header, &ResourceDescription>(hob_header);
          if resource_hob.resource_type == RESOURCE_SYSTEM_MEMORY {
            ALLOCATOR.lock().add_initial_allocation(
                MemoryType::ConventionalMemory,
                resource_hob.resource_length / PAGE_SIZE,
                resource_hob.physical_start,
                MEMORY_WB,
                );
          }
        }
        HOB_TYPE_END_OF_HOB_LIST => {
          break;
        }
        _ => {}
      }
      let addr = hob_header as usize + header.length as usize;
      hob_header = addr as *const Header;
    }
  }

  unsafe {
    let mut hob_header : *const Header = hob as *const Header;

    loop {
      let header = transmute::<*const Header, &Header>(hob_header);
      match header.r#type {
        HOB_TYPE_MEMORY_ALLOCATION => {
          let allocation_hob = transmute::<*const Header, &MemoryAllocation>(hob_header);
          ALLOCATOR.lock().allocate_pages(
              AllocateType::AllocateAddress,
              allocation_hob.alloc_descriptor.memory_type,
              allocation_hob.alloc_descriptor.memory_length / PAGE_SIZE,
              allocation_hob.alloc_descriptor.memory_base_address,
              );
        }
        HOB_TYPE_END_OF_HOB_LIST => {
          break;
        }
        _ => {}
      }
      let addr = hob_header as usize + header.length as usize;
      hob_header = addr as *const Header;
    }
  }
}

#[cfg(not(test))]
#[no_mangle]
pub extern "win64" fn _start(hob: *const c_void) -> ! {

    log!("Starting UEFI hob - {:p}\n", hob);

    pi::hob_lib::dump_hob (hob);

    initialize_memory(hob);

    enable_sse2();

    efi::enter_uefi(hob);

    //i8042_reset();
}
