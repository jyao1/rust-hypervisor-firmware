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
use core::option::Option;

use crate::efi::peloader::*;

const HANDLE_SIGNATURE: u32 = 0x4C444849; // 'I','H','D','L'

#[repr(C)]
#[derive(Debug, Default, Clone)]
struct ProtocolStruct {
    guid : usize,
    interface : usize,
}

const MAX_PROTOCOL_STRUCT: usize = 16;

#[repr(C)]
#[derive(Debug, Default, Clone)]
struct ProtocolHandle {
    signature: u32,
    protocol_count: usize,
    protocol_struct : [ProtocolStruct; MAX_PROTOCOL_STRUCT],
}

const MAX_HANDLE_STRUCT: usize = 16;

#[repr(C)]
#[derive(Debug, Default, Clone)]
pub struct HandleDatabase {
    protocol_handle_count: usize,
    protocol_handle : [ProtocolHandle; MAX_HANDLE_STRUCT],
}

impl HandleDatabase {
    pub fn install_protocol (
        &mut self,
        handle: Handle,
        guid : *mut Guid,
        interface : *mut c_void,
    ) -> (Status, Handle) {
        let (status, mut cur_handle) = self.get_handle (handle);
        match status {
          Status::SUCCESS => {},
          Status::NOT_FOUND => {
            let (status, new_handle) = self.get_new_handle ();
            match status {
              Status::SUCCESS => {},
              _ => {return (status, core::ptr::null_mut());},
            }
            cur_handle = new_handle;
          },
          _ => {return (status, core::ptr::null_mut());},
        }
        assert!(cur_handle != core::ptr::null_mut());
        let protocol_handle = unsafe {transmute::<Handle, &mut ProtocolHandle>(cur_handle)};
        assert!(protocol_handle.signature == HANDLE_SIGNATURE);

        let (status, mut cur_protocol_struct) = self.get_protocol (protocol_handle, guid);
        match status {
          Status::SUCCESS => {return (Status::INVALID_PARAMETER, core::ptr::null_mut())},
          Status::NOT_FOUND => {
            let (status, new_protocol_struct) = self.get_new_protocol (protocol_handle);
            match status {
              Status::SUCCESS => {},
              _ => {return (status, core::ptr::null_mut());},
            }
            cur_protocol_struct = new_protocol_struct;
          },
          _ => {return (status, core::ptr::null_mut());},
        }

        let protocol_struct = unsafe {transmute::<*mut ProtocolStruct, &mut ProtocolStruct>(cur_protocol_struct)};
        protocol_struct.guid = guid as usize;
        protocol_struct.interface = interface as usize;

        (Status::SUCCESS, cur_handle)
    }

    fn get_new_protocol (
        &mut self,
        protocol_handle : &mut ProtocolHandle
        ) -> (Status, *mut ProtocolStruct) {
        if (protocol_handle.protocol_count >= MAX_PROTOCOL_STRUCT) {
          return (Status::OUT_OF_RESOURCES, core::ptr::null_mut());
        }
        let protocol_struct = &mut protocol_handle.protocol_struct[protocol_handle.protocol_count];
        protocol_handle.protocol_count = protocol_handle.protocol_count + 1;

        protocol_struct.guid = 0;
        protocol_struct.interface = 0;

        (Status::SUCCESS, protocol_struct as *mut ProtocolStruct)
    }
    
    fn get_protocol (
        &mut self,
        protocol_handle : &mut ProtocolHandle,
        guid : *mut Guid,
        ) -> (Status, *mut ProtocolStruct) {
        assert!(protocol_handle.signature == HANDLE_SIGNATURE);
        for index in 0 .. protocol_handle.protocol_count {
          let guid_addr = protocol_handle.protocol_struct[index].guid;
          let guid_ptr : *mut Guid = guid_addr as *mut c_void as *mut Guid;
          let guid1_data = unsafe {(*guid).as_fields()};
          let guid2_data = unsafe {(*guid_ptr).as_fields()};
          if guid1_data == guid2_data {
            return (Status::SUCCESS, &mut protocol_handle.protocol_struct[index]);
          }
        }
        (Status::NOT_FOUND, core::ptr::null_mut())
    }

    fn get_new_handle (
        &mut self
        ) -> (Status, Handle) {
        if (self.protocol_handle_count >= MAX_HANDLE_STRUCT) {
          return (Status::OUT_OF_RESOURCES, core::ptr::null_mut());
        }
        let protocol_handle = &mut self.protocol_handle[self.protocol_handle_count];
        self.protocol_handle_count = self.protocol_handle_count + 1;

        protocol_handle.signature = HANDLE_SIGNATURE;
        protocol_handle.protocol_count = 0;

        (Status::SUCCESS, protocol_handle as *mut ProtocolHandle as Handle)
    }

    fn get_handle (
        &mut self,
        handle : Handle,
    ) -> (Status, Handle) {
        if handle == core::ptr::null_mut() {
          return (Status::NOT_FOUND, core::ptr::null_mut());
        }
    
        let protocol_handle = unsafe {transmute::<Handle, &mut ProtocolHandle>(handle)};
        if protocol_handle.signature != HANDLE_SIGNATURE {
          return (Status::INVALID_PARAMETER, core::ptr::null_mut())
        }

        return (Status::SUCCESS, handle);
    }

    pub fn new() -> HandleDatabase {
        HandleDatabase {
            protocol_handle_count: 0,
            ..HandleDatabase::default()
        }
        
    }
}

