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

use r_efi::protocols::device_path::Protocol as DevicePathProtocol;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct MemoryMaped {
    pub header: DevicePathProtocol,
    pub memory_type: MemoryType,
    pub start_address: PhysicalAddress,
    pub end_address: PhysicalAddress,
}

pub fn get_device_path_node_size (
       device_path: *mut DevicePathProtocol,
       ) -> usize
{
    unsafe {
      let size = ((*device_path).length[0] as usize) | (((*device_path).length[1] as usize) << 8);
      size
    }
}

pub fn get_device_path_node_type (
       device_path: *mut DevicePathProtocol,
       ) -> u8
{
    unsafe {
      (*device_path).r#type
    }
}

pub fn get_device_path_node_sub_type (
       device_path: *mut DevicePathProtocol,
       ) -> u8
{
    unsafe {
      (*device_path).sub_type
    }
}

pub fn get_next_device_path_node (
       device_path: *mut DevicePathProtocol,
       ) -> *mut DevicePathProtocol
{
  let size : usize = get_device_path_node_size(device_path);
  (device_path as usize + size) as *mut DevicePathProtocol
}

pub fn get_device_path_size (
       device_path: *mut DevicePathProtocol
       ) -> usize
{
    let mut size: usize = 0;
    let mut device_path_node = device_path;
    loop {
      let node_size = get_device_path_node_size(device_path_node);
      size = size + node_size;
      if get_device_path_node_type(device_path_node) == r_efi::protocols::device_path::TYPE_END && 
         get_device_path_node_sub_type(device_path_node) == r_efi::protocols::device_path::End::SUBTYPE_ENTIRE {
        break;
      }
      device_path_node = get_next_device_path_node (device_path_node);
    }
    size
}