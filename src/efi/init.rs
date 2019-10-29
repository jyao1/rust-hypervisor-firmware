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

use r_efi::protocols::simple_file_system::Protocol as SimpleFileSystemProtocol;

use core::ffi::c_void;
use core::mem::transmute;
use core::mem::size_of;

use crate::pi::hob::*;
use crate::pi::fv_lib::*;
use crate::mem::MemoryRegion;

use crate::efi::alloc::Allocator;
use crate::efi::ALLOCATOR;
use crate::efi::PAGE_SIZE;

use crate::pci;
use crate::part;
use crate::fat;
use crate::efi::file;

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

pub fn initialize_console(system_table: *mut efi::SystemTable, con_in_ex: *mut c_void) {
  unsafe {
    let status = crate::efi::install_protocol_interface (
                       &mut (*system_table).console_in_handle as *mut Handle,
                       &mut r_efi::protocols::simple_text_input::PROTOCOL_GUID as *mut Guid,
                       InterfaceType::NativeInterface,
                       (*system_table).con_in as *mut c_void
                       );
    let status = crate::efi::install_protocol_interface (
                       &mut (*system_table).console_in_handle as *mut Handle,
                       &mut r_efi::protocols::simple_text_input_ex::PROTOCOL_GUID as *mut Guid,
                       InterfaceType::NativeInterface,
                       con_in_ex
                       );
    let status = crate::efi::install_protocol_interface (
                       &mut (*system_table).console_out_handle as *mut Handle,
                       &mut r_efi::protocols::simple_text_output::PROTOCOL_GUID as *mut Guid,
                       InterfaceType::NativeInterface,
                       (*system_table).con_out as *mut c_void
                       );
    let status = crate::efi::install_protocol_interface (
                       &mut (*system_table).standard_error_handle as *mut Handle,
                       &mut r_efi::protocols::simple_text_output::PROTOCOL_GUID as *mut Guid,
                       InterfaceType::NativeInterface,
                       (*system_table).std_err as *mut c_void
                       );
  }
}

#[cfg(not(test))]
const VIRTIO_PCI_VENDOR_ID: u16 = 0x1af4;
#[cfg(not(test))]
const VIRTIO_PCI_BLOCK_DEVICE_ID: u16 = 0x1042;

pub fn initialize_fs() {
    pci::print_bus();

    let mut pci_transport;
    let mut device;
    match pci::search_bus(VIRTIO_PCI_VENDOR_ID, VIRTIO_PCI_BLOCK_DEVICE_ID) {
      Some(pci_device) => {
        pci_transport = pci::VirtioPciTransport::new(pci_device);
        device = crate::block::VirtioBlockDevice::new(&mut pci_transport);
      },
      _ => {
        return ;
      }
    }

    match device.init() {
        Err(_) => {
            log!("Error configuring block device\n");
            return ;
        }
        Ok(_) => log!(
            "Virtio block device configured. Capacity: {} sectors\n",
            device.get_capacity()
        ),
    }

    let mut f;

    match part::find_efi_partition(&device) {
        Ok((start, end)) => {
            log!("Found EFI partition\n");
            f = fat::Filesystem::new(&device, start, end);
            if f.init().is_err() {
                log!("Failed to create filesystem\n");
                return ;
            }
        }
        Err(_) => {
            log!("Failed to find EFI partition\n");
            return ;
        }
    }

    log!("Filesystem ready\n");


    let efi_part_id = unsafe { crate::efi::block::populate_block_wrappers(&mut crate::efi::BLOCK_WRAPPERS, &device) };

    let mut wrapped_fs = file::FileSystemWrapper::new(&f, efi_part_id);

    let mut handle : Handle = core::ptr::null_mut();
    let status = crate::efi::install_protocol_interface (
                       &mut handle as *mut Handle,
                       &mut r_efi::protocols::simple_file_system::PROTOCOL_GUID as *mut Guid,
                       InterfaceType::NativeInterface,
                       &mut wrapped_fs.proto as *mut SimpleFileSystemProtocol as *mut c_void
                       );
    if status != Status::SUCCESS {
      return ;
    }
    log!("Filesystem installed\n");
}
 