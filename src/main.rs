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

#![feature(llvm_asm)]
#![feature(abi_x86_interrupt)]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]
#![cfg_attr(test, allow(unused_imports))]

#[macro_use]
extern crate lazy_static;

extern crate x86_64;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use x86_64::{
    instructions::hlt,
    registers::control::{Cr0, Cr0Flags, Cr4, Cr4Flags},
};


#[macro_use]
mod logger;

#[macro_use]
mod common;

use core::panic::PanicInfo;

use core::ffi::c_void;

use cpuio::Port;

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
mod calloc;

#[cfg(not(test))]
#[panic_handler]
#[allow(clippy::empty_loop)]
fn panic(_info: &PanicInfo) -> ! {
    log!("panic ... {:?}\n", _info);
    loop {}
}

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt.invalid_opcode.set_handler_fn(invalid_opcode_handler);
        idt.general_protection_fault.set_handler_fn(general_protection_fault_handler);
        idt
    };
}

extern "x86-interrupt" fn page_fault_handler(stack_frame: &mut InterruptStackFrame, error_code: PageFaultErrorCode) {
    log!("EXCEPTION: PAGE FAULT {:#?}\n{:#?}", error_code, stack_frame);
    loop {}
}

extern "x86-interrupt" fn general_protection_fault_handler(stack_frame: &mut InterruptStackFrame, error_code: u64) {
    log!("EXCEPTION: GENERAL PROTECTION FAULT {:?}\n{:#?}", error_code, stack_frame);
    loop {}
}

extern "x86-interrupt" fn invalid_opcode_handler(stack_frame: &mut InterruptStackFrame) {
    log!("EXCEPTION: INVALID OPCODE FAULT \n{:#?}", stack_frame);
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
    // unsafe {
    //     llvm_asm!("movq %cr0, %rax");
    //     llvm_asm!("or $$0x2, %ax");
    //     llvm_asm!("movq %rax, %cr0");
    //     llvm_asm!("movq %cr4, %rax");
    //     llvm_asm!("or $$0x600, %ax");
    //     llvm_asm!("movq %rax, %cr4");
    // }
    let mut cr0 = Cr0::read();
    cr0.remove(Cr0Flags::EMULATE_COPROCESSOR);
    cr0.insert(Cr0Flags::MONITOR_COPROCESSOR);
    unsafe { Cr0::write(cr0) };
    let mut cr4 = Cr4::read();
    cr4.insert(Cr4Flags::OSFXSR);
    cr4.insert(Cr4Flags::OSXMMEXCPT_ENABLE);
    unsafe { Cr4::write(cr4) };
}

#[cfg(not(test))]
#[no_mangle]
pub extern "win64" fn _start(hob: *const c_void) -> ! {

    log!("Starting UEFI hob - {:p}\n", hob);

    IDT.load();
    enable_sse2();

    efi::enter_uefi(hob);

    //i8042_reset();
}
