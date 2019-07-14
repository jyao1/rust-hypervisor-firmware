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

use r_efi::efi;
use r_efi::efi::{
    AllocateType, Boolean, CapsuleHeader, Char16, Event, EventNotify, Guid, Handle, InterfaceType,
    LocateSearchType, MemoryDescriptor, MemoryType, OpenProtocolInformationEntry, PhysicalAddress,
    ResetType, Status, Time, TimeCapabilities, TimerDelay, Tpl, MEMORY_WB
};
use r_efi::system::{VARIABLE_NON_VOLATILE, VARIABLE_BOOTSERVICE_ACCESS, VARIABLE_RUNTIME_ACCESS};

use core::ffi::c_void;
use core::mem::transmute;
use core::mem::size_of;

use crate::pi::hob::*;
use crate::pi::fv_lib::*;
use crate::mem::MemoryRegion;

use crate::efi::alloc::Allocator;
use crate::efi::ALLOCATOR;
use crate::efi::PAGE_SIZE;

#[cfg(not(test))]
pub fn initialize_memory(hob: *const c_void) {

  let mut hob_header : *const Header = hob as *const Header;
  loop {
    let header = unsafe {transmute::<*const Header, &Header>(hob_header)};
    match header.r#type {
      HOB_TYPE_RESOURCE_DESCRIPTOR => {
        let resource_hob = unsafe {transmute::<*const Header, &ResourceDescription>(hob_header)};
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


  let mut hob_header : *const Header = hob as *const Header;
  loop {
    let header = unsafe {transmute::<*const Header, &Header>(hob_header)};
    match header.r#type {
      HOB_TYPE_MEMORY_ALLOCATION => {
        let allocation_hob = unsafe {transmute::<*const Header, &MemoryAllocation>(hob_header)};
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

#[cfg(not(test))]
pub fn find_loader(hob: *const c_void) -> (*const c_void, usize) {
  let (image, size) = find_image_in_fv (hob);
  (image, size)
}

#[cfg(not(test))]
pub fn initialize_variable() {
  let mut var_name: [Char16; 13] = [0x50, 0x6c, 0x61, 0x74, 0x66, 0x6F, 0x72, 0x6d, 0x4c, 0x61, 0x6e, 0x67, 0x00]; // L"PlatformLang"
  let mut var_data: [u8; 3] = [0x65, 0x6e, 0x00]; // "en"
  crate::efi::set_variable(
    &mut var_name as *mut [Char16; 13] as *mut Char16,
    &mut crate::efi::variable::GLOBAL_VARIABLE_GUID as *mut Guid,
    VARIABLE_NON_VOLATILE | VARIABLE_BOOTSERVICE_ACCESS | VARIABLE_RUNTIME_ACCESS,
    3,
    &mut var_data as *mut [u8; 3] as *mut c_void
    );
}

// 