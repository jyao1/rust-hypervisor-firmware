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

use core::ffi::c_void;
use core::mem::transmute;
use core::mem::size_of;

use crate::efi::peloader::*;

pub const IMAGE_INFO_GUID: Guid = Guid::from_fields(
    0xdecf2644, 0xbc0, 0x4840, 0xb5, 0x99, &[0x13, 0x4b, 0xee, 0xa, 0x9e, 0x71]
);

const IMAGE_INFO_SIGNATURE: u32 = 0x49444849; // 'I','H','D','I'

#[derive(Default)]
struct ImageInfo {
    signature: u32,
    source_buffer: usize,
    source_size: usize,
    image_address: usize,
    image_size: usize,
    entry_point: usize,
}

#[derive(Default)]
pub struct Image {
    image_count: usize,
}

impl Image {
    pub fn load_image (
        &mut self,
        source_buffer: *mut c_void,
        source_size: usize,
    ) -> (Status, Handle) {
        let mut handle_address: *mut c_void = core::ptr::null_mut();

        let status = crate::efi::allocate_pool (MemoryType::BootServicesData, size_of::<ImageInfo>() as usize, &mut handle_address);
        if status != Status::SUCCESS {
          log!("load_image - fail on allocate pool\n");
          return (status, core::ptr::null_mut())
        }

        let handle = unsafe {transmute::<*mut c_void, &mut ImageInfo>(handle_address)};
        handle.signature = IMAGE_INFO_SIGNATURE;
        handle.source_buffer = source_buffer as usize;
        handle.source_size   = source_size;

        handle.image_size = peloader_get_image_info (source_buffer, source_size);
        log!("load_image - image_size 0x{:x}\n", handle.image_size);
        if handle.image_size == 0 {
          return (Status::SECURITY_VIOLATION, core::ptr::null_mut())
        }
        let mut image_address : *mut c_void = core::ptr::null_mut();
        let status = crate::efi::allocate_pool (MemoryType::BootServicesData, handle.image_size, &mut image_address);
        if status != Status::SUCCESS {
          log!("load_image - fail on allocate pool\n");
          return (Status::OUT_OF_RESOURCES, core::ptr::null_mut())
        }
        handle.image_address = image_address as usize;
        log!("image_address - 0x{:x}\n", handle.image_address);

        handle.entry_point = peloader_load_image (image_address, handle.image_size, source_buffer, source_size);
        log!("entry_point - 0x{:x}\n", handle.entry_point);
        if handle.entry_point == 0 {
          return (Status::SECURITY_VIOLATION, core::ptr::null_mut())
        }

        let mut image_handle : Handle = core::ptr::null_mut();
        let status = crate::efi::install_protocol_interface (
                       &mut image_handle,
                       &mut IMAGE_INFO_GUID as *mut Guid,
                       InterfaceType::NativeInterface,
                       handle_address
                       );

        (status, image_handle)
    }
    pub fn start_image (
        &mut self,
        image_handle: Handle,
    ) -> (Status, usize, *mut Char16) {

        let mut handle_address: *mut c_void = core::ptr::null_mut();
        let status = crate::efi::handle_protocol (
                       image_handle,
                       &mut IMAGE_INFO_GUID,
                       &mut handle_address
                       );
        if status != Status::SUCCESS {
          return (Status::INVALID_PARAMETER, 0, core::ptr::null_mut())
        }

        let handle = unsafe {transmute::<*mut c_void, &mut ImageInfo>(handle_address)};
        if handle.signature != IMAGE_INFO_SIGNATURE {
          return (Status::INVALID_PARAMETER, 0, core::ptr::null_mut())
        }

        log!("start_image - entry_point 0x{:x}\n", handle.entry_point);

        let ptr = handle.entry_point as *const ();
        let code: extern "win64" fn(Handle, *mut efi::SystemTable) -> Status = unsafe { core::mem::transmute(ptr) };

        let status = unsafe { (code)(image_handle, &mut crate::efi::ST) };

        (status, 0, core::ptr::null_mut())
    }

    pub fn new() -> Image {
        Image {
            image_count: 0,
        }
    }
}

