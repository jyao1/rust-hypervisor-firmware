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

mod alloc;
mod file;
mod device_path;
mod image;
mod handle_database;
mod variable;
mod peloader;
mod init;

use lazy_static::lazy_static;
use spin::Mutex;

use r_efi::efi;
use r_efi::efi::{
    AllocateType, Boolean, CapsuleHeader, Char16, Event, EventNotify, Guid, Handle, InterfaceType,
    LocateSearchType, MemoryDescriptor, MemoryType, OpenProtocolInformationEntry, PhysicalAddress,
    ResetType, Status, Time, TimeCapabilities, TimerDelay, Tpl, OPEN_PROTOCOL_GET_PROTOCOL
};

use r_efi::protocols::simple_text_input::InputKey;
use r_efi::protocols::simple_text_input::Protocol as SimpleTextInputProtocol;
use r_efi::protocols::simple_text_output::Mode as SimpleTextOutputMode;
use r_efi::protocols::simple_text_output::Protocol as SimpleTextOutputProtocol;
//use r_efi::protocols::loaded_image::Protocol as LoadedImageProtocol;
use r_efi::protocols::device_path::Protocol as DevicePathProtocol;
use crate::efi::device_path::MemoryMaped as MemoryMappedDevicePathProtocol;

use r_efi::{eficall, eficall_abi};

use core::ffi::c_void;

use crate::pi::hob::{
  Header, MemoryAllocation, ResourceDescription,
  RESOURCE_SYSTEM_MEMORY, HOB_TYPE_MEMORY_ALLOCATION, HOB_TYPE_RESOURCE_DESCRIPTOR, HOB_TYPE_END_OF_HOB_LIST
  };

use alloc::Allocator;
use handle_database::HandleDatabase;
use variable::Variable;
use variable::MAX_VARIABLE_NAME;
use variable::MAX_VARIABLE_DATA;
use image::Image;

lazy_static! {
    pub static ref ALLOCATOR: Mutex<Allocator> = Mutex::new(Allocator::new());
}

lazy_static! {
    pub static ref HANDLE_DATABASE: Mutex<HandleDatabase> = Mutex::new(HandleDatabase::new());
}

lazy_static! {
    pub static ref VARIABLE: Mutex<Variable> = Mutex::new(Variable::new());
}

lazy_static! {
    pub static ref IMAGE: Mutex<Image> = Mutex::new(Image::new());
}

pub fn print_guid (
    guid: *mut Guid,
    )
{
    let guid_data = unsafe { (*guid).as_fields() };
    crate::log!(
      "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
      guid_data.0,
      guid_data.1,
      guid_data.2,
      guid_data.3,
      guid_data.4,
      guid_data.5[0],
      guid_data.5[1],
      guid_data.5[2],
      guid_data.5[3],
      guid_data.5[4],
      guid_data.5[5]
      );
}

#[cfg(not(test))]
pub extern "win64" fn stdin_reset(_: *mut SimpleTextInputProtocol, _: Boolean) -> Status {
    crate::log!("EFI_STUB: stdin_reset\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn stdin_read_key_stroke(
    _: *mut SimpleTextInputProtocol,
    _: *mut InputKey,
) -> Status {
    crate::log!("EFI_STUB: stdin_read_key_stroke\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn stdout_reset(_: *mut SimpleTextOutputProtocol, _: Boolean) -> Status {
    crate::log!("EFI_STUB: stdout_reset\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn stdout_output_string(
    _: *mut SimpleTextOutputProtocol,
    message: *mut Char16,
) -> Status {
    let mut string_end = false;

    loop {
        let mut output: [u8; 128] = [0; 128];
        let mut i: usize = 0;
        while i < output.len() {
            output[i] = (unsafe { *message.add(i) } & 0xffu16) as u8;
            if output[i] == 0 {
                string_end = true;
                break;
            }
            i += 1;
        }
        crate::log!("{}", unsafe { core::str::from_utf8_unchecked(&output) });
        if string_end {
            break;
        }
    }
    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn stdout_test_string(
    _: *mut SimpleTextOutputProtocol,
    message: *mut Char16,
) -> Status {
    crate::log!("EFI_STUB: stdout_test_string\n");

    let mut string_end = false;

    loop {
        let mut output: [u8; 128] = [0; 128];
        let mut i: usize = 0;
        while i < output.len() {
            output[i] = (unsafe { *message.add(i) } & 0xffu16) as u8;
            if output[i] == 0 {
                string_end = true;
                break;
            }
            i += 1;
        }
        crate::log!("{}", unsafe { core::str::from_utf8_unchecked(&output) });
        if string_end {
            break;
        }
    }

    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn stdout_query_mode(
    _: *mut SimpleTextOutputProtocol,
    mode_number: usize,
    columns: *mut usize,
    raws: *mut usize,
) -> Status {
    crate::log!("EFI_STUB: stdout_query_mode - {}\n", mode_number);
    if mode_number != 0 && mode_number != 1 {
      return Status::UNSUPPORTED;
    }
    if columns == core::ptr::null_mut() || raws == core::ptr::null_mut() {
      return Status::INVALID_PARAMETER;
    }
    if mode_number == 0 {
      unsafe {
        *columns = 80;
        *raws = 25;
      }
    }
    if mode_number == 1 {
      unsafe {
        *columns = 80;
        *raws = 50;
      }
    }
    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn stdout_set_mode(_: *mut SimpleTextOutputProtocol, _: usize) -> Status {
    crate::log!("EFI_STUB: stdout_set_mode\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn stdout_set_attribute(_: *mut SimpleTextOutputProtocol, _: usize) -> Status {
    crate::log!("EFI_STUB: stdout_set_attribute\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn stdout_clear_screen(_: *mut SimpleTextOutputProtocol) -> Status {
    crate::log!("EFI_STUB: stdout_clear_screen\n");
    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn stdout_set_cursor_position(
    _: *mut SimpleTextOutputProtocol,
    _: usize,
    _: usize,
) -> Status {
    crate::log!("EFI_STUB: stdout_set_cursor_position\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn stdout_enable_cursor(_: *mut SimpleTextOutputProtocol, _: Boolean) -> Status {
    crate::log!("EFI_STUB: stdout_enable_cursor\n");
    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn get_time(_: *mut Time, _: *mut TimeCapabilities) -> Status {
    crate::log!("EFI_STUB: get_time\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn set_time(_: *mut Time) -> Status {
    crate::log!("EFI_STUB: set_time\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn get_wakeup_time(_: *mut Boolean, _: *mut Boolean, _: *mut Time) -> Status {
    crate::log!("EFI_STUB: get_wakeup_time\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn set_wakeup_time(_: Boolean, _: *mut Time) -> Status {
    crate::log!("EFI_STUB: set_wakeup_time\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn set_virtual_address_map(
    map_size: usize,
    descriptor_size: usize,
    version: u32,
    descriptors: *mut MemoryDescriptor,
) -> Status {
    let count = map_size / descriptor_size;

    if version != efi::MEMORY_DESCRIPTOR_VERSION {
        return Status::INVALID_PARAMETER;
    }

    let descriptors = unsafe {
        core::slice::from_raw_parts_mut(descriptors as *mut alloc::MemoryDescriptor, count)
    };

    ALLOCATOR.lock().update_virtual_addresses(descriptors)
}

#[cfg(not(test))]
pub extern "win64" fn convert_pointer(_: usize, _: *mut *mut c_void) -> Status {
    crate::log!("EFI_STUB: convert_pointer\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn get_variable(
    var_name: *mut Char16,
    var_guid: *mut Guid,
    attributes: *mut u32,
    size: *mut usize,
    data: *mut core::ffi::c_void,
) -> Status {
    crate::log!("EFI_STUB: get_variable ");

    let mut string_end = false;
    let mut name_buffer: [u8; MAX_VARIABLE_NAME] = [0; MAX_VARIABLE_NAME];
    let mut name_len: usize = 0;
    while name_len < MAX_VARIABLE_NAME {
      name_buffer[name_len] = (unsafe { *var_name.add(name_len) } & 0xffu16) as u8;
      crate::log!("{}", name_buffer[name_len] as char);
      if name_buffer[name_len] == 0 {
        string_end = true;
        break;
      }
      name_len += 1;
    }
    crate::log!(" ");
    print_guid (var_guid);
    crate::log!("\n");
    let guid_buffer : *mut [u8; 16] = unsafe { core::mem::transmute::<*mut Guid, *mut [u8; 16]>(var_guid) };

    if !string_end {
      crate::log!("name too long\n");
      return Status::UNSUPPORTED;
    }

    let (status, var_attributes, var_size, var_data) = VARIABLE.lock().get_variable(
                     &mut name_buffer as *mut [u8; MAX_VARIABLE_NAME],
                     guid_buffer
                     );

    if (status == Status::NOT_FOUND) {
      return status;
    }

    if unsafe {*size} < var_size {
      unsafe {*size = var_size;}
      return Status::BUFFER_TOO_SMALL;
    }

    unsafe {*size = var_size;}
    let data_ptr : *mut c_void = unsafe { core::mem::transmute::<*mut [u8; MAX_VARIABLE_DATA], *mut c_void>(var_data) };
    unsafe {core::ptr::copy_nonoverlapping (data_ptr, data, var_size);}

    if attributes != core::ptr::null_mut() {
      unsafe {*attributes = var_attributes;}
    }

    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn get_next_variable_name(
    _: *mut usize,
    _: *mut Char16,
    _: *mut Guid,
) -> Status {
    crate::log!("EFI_STUB: get_next_variable\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn set_variable(
    var_name: *mut Char16,
    var_guid: *mut Guid,
    attributes: u32,
    size: usize,
    data: *mut c_void,
) -> Status {
    crate::log!("EFI_STUB: set_variable");
    
    let mut string_end = false;
    let mut name_buffer: [u8; MAX_VARIABLE_NAME] = [0; MAX_VARIABLE_NAME];
    let mut name_len: usize = 0;
    while name_len < MAX_VARIABLE_NAME {
      name_buffer[name_len] = (unsafe { *var_name.add(name_len) } & 0xffu16) as u8;
      crate::log!("{}", name_buffer[name_len] as char);
      if name_buffer[name_len] == 0 {
        string_end = true;
        break;
      }
      name_len += 1;
    }

    let guid_data = unsafe { (*var_guid).as_fields() };
    crate::log!(" ");
    print_guid (var_guid);
    crate::log!("\n");
    let guid_buffer : *mut [u8; 16] = unsafe { core::mem::transmute::<*mut Guid, *mut [u8; 16]>(var_guid) };

    if !string_end {
      crate::log!("name too long\n");
      return Status::UNSUPPORTED;
    }

    if size > MAX_VARIABLE_DATA {
      crate::log!("data too long\n");
      return Status::UNSUPPORTED;
    }

    let data_buffer: *mut [u8; MAX_VARIABLE_DATA] = unsafe { core::mem::transmute::<*mut c_void, *mut [u8; MAX_VARIABLE_DATA]>(data) };

    let (status) = VARIABLE.lock().set_variable(
                     &mut name_buffer as *mut [u8; MAX_VARIABLE_NAME],
                     guid_buffer,
                     attributes,
                     size,
                     data_buffer
                     );

    status
}

#[cfg(not(test))]
pub extern "win64" fn get_next_high_mono_count(_: *mut u32) -> Status {
    crate::log!("EFI_STUB: get_next_high_mono_count\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn reset_system(_: ResetType, _: Status, _: usize, _: *mut c_void) {
    crate::i8042_reset();
}

#[cfg(not(test))]
pub extern "win64" fn update_capsule(
    _: *mut *mut CapsuleHeader,
    _: usize,
    _: PhysicalAddress,
) -> Status {
    crate::log!("EFI_STUB: update_capsule\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn query_capsule_capabilities(
    _: *mut *mut CapsuleHeader,
    _: usize,
    _: *mut u64,
    _: *mut ResetType,
) -> Status {
    crate::log!("EFI_STUB: query_capsule_capabilities\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn query_variable_info(_: u32, _: *mut u64, _: *mut u64, _: *mut u64) -> Status {
    crate::log!("EFI_STUB: query_variable_info\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn raise_tpl(_: Tpl) -> Tpl {
    crate::log!("EFI_STUB: raise_tpl\n");
    0
}

#[cfg(not(test))]
pub extern "win64" fn restore_tpl(_: Tpl) {
    crate::log!("EFI_STUB: restore_tpl\n");
}

#[cfg(not(test))]
pub extern "win64" fn allocate_pages(
    allocate_type: AllocateType,
    memory_type: MemoryType,
    pages: usize,
    address: *mut PhysicalAddress,
) -> Status {
    let (status, new_address) =
        ALLOCATOR
            .lock()
            .allocate_pages(
                allocate_type,
                memory_type,
                pages as u64,
                unsafe { *address } as u64,
            );
    if status == Status::SUCCESS {
        unsafe {
            *address = new_address;
        }
    } else {
      log!("allocate pages status - {:?}\n", status);
    }
    status
}

#[cfg(not(test))]
pub extern "win64" fn free_pages(address: PhysicalAddress, _: usize) -> Status {
    ALLOCATOR.lock().free_pages(address)
}

#[cfg(not(test))]
pub extern "win64" fn get_memory_map(
    memory_map_size: *mut usize,
    out: *mut MemoryDescriptor,
    key: *mut usize,
    descriptor_size: *mut usize,
    descriptor_version: *mut u32,
) -> Status {
    let count = ALLOCATOR.lock().get_descriptor_count();
    let map_size = core::mem::size_of::<MemoryDescriptor>() * count;
    if unsafe { *memory_map_size } < map_size {
        unsafe {
            *memory_map_size = map_size;
        }
        return Status::BUFFER_TOO_SMALL;
    }

    let out =
        unsafe { core::slice::from_raw_parts_mut(out as *mut alloc::MemoryDescriptor, count) };
    let count = ALLOCATOR.lock().get_descriptors(out);
    let map_size = core::mem::size_of::<MemoryDescriptor>() * count;
    unsafe {
        *memory_map_size = map_size;
        *descriptor_version = efi::MEMORY_DESCRIPTOR_VERSION;
        *descriptor_size = core::mem::size_of::<MemoryDescriptor>();
        *key = ALLOCATOR.lock().get_map_key();
    }

    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn allocate_pool(
    memory_type: MemoryType,
    size: usize,
    address: *mut *mut c_void,
) -> Status {
    let (status, new_address) = ALLOCATOR.lock().allocate_pages(
        AllocateType::AllocateAnyPages,
        memory_type,
        ((size + PAGE_SIZE as usize - 1) / PAGE_SIZE as usize) as u64,
        address as u64,
    );

    if status == Status::SUCCESS {
        unsafe {
            *address = new_address as *mut c_void;
        }
    } else {
      log!("allocate pool status - {:?}\n", status);
    }

    status
}

#[cfg(not(test))]
pub extern "win64" fn free_pool(ptr: *mut c_void) -> Status {
    ALLOCATOR.lock().free_pages(ptr as u64)
}

#[cfg(not(test))]
pub extern "win64" fn create_event(
    _: u32,
    _: Tpl,
    _: EventNotify,
    _: *mut c_void,
    _: *mut Event,
) -> Status {
    crate::log!("EFI_STUB: create_event\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn set_timer(_: Event, _: TimerDelay, _: u64) -> Status {
    crate::log!("EFI_STUB: set_timer\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn wait_for_event(_: usize, _: *mut Event, _: *mut usize) -> Status {
    crate::log!("EFI_STUB: wait_for_event\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn signal_event(_: Event) -> Status {
    crate::log!("EFI_STUB: signal_event\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn close_event(_: Event) -> Status {
    crate::log!("EFI_STUB: close_event\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn check_event(_: Event) -> Status {
    crate::log!("EFI_STUB: check_event\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn install_protocol_interface(
    handle: *mut Handle,
    guid: *mut Guid,
    interface_type: InterfaceType,
    interface: *mut c_void,
) -> Status {
    crate::log!("EFI_STUB: install_protocol_interface - ");
    print_guid (guid);
    crate::log!("\n");

    let (status, new_handle) = HANDLE_DATABASE.lock().install_protocol(
                unsafe {*handle},
                guid,
                interface,
            );
    log!("status - {:?}\n", status);
    if status == Status::SUCCESS {
        unsafe {
            *handle = new_handle;
        }
    }
    status
}

#[cfg(not(test))]
pub extern "win64" fn reinstall_protocol_interface(
    _: Handle,
    _: *mut Guid,
    _: *mut c_void,
    _: *mut c_void,
) -> Status {
    crate::log!("EFI_STUB: reinstall_protocol_interface\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn uninstall_protocol_interface(
    _: Handle,
    _: *mut Guid,
    _: *mut c_void,
) -> Status {
    crate::log!("EFI_STUB: uninstall_protocol_interface\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn handle_protocol(
    handle: Handle,
    guid: *mut Guid,
    out: *mut *mut c_void,
) -> Status {
    if guid == core::ptr::null_mut() {
        crate::log!("EFI_STUB: handle_protocol - NULL\n");
        return Status::INVALID_PARAMETER;
    }

    crate::log!("EFI_STUB: handle_protocol - ");
    print_guid (guid);
    crate::log!("\n");

    if unsafe { *guid } == r_efi::protocols::loaded_image::PROTOCOL_GUID {
        unsafe {
            *out = handle;
        }
        return Status::SUCCESS;
    }
    if unsafe { *guid } == r_efi::protocols::simple_file_system::PROTOCOL_GUID {
        unsafe {
            *out = handle;
        }
        return Status::SUCCESS;
    }

    let (status, interface) = HANDLE_DATABASE.lock().handle_protocol(handle, guid);
    log!("status - {:?}\n", status);
    if status == Status::SUCCESS {
        unsafe {
            *out = interface;
        }
    }
    status
}

#[cfg(not(test))]
pub extern "win64" fn register_protocol_notify(
    _: *mut Guid,
    _: Event,
    _: *mut *mut c_void,
) -> Status {
    crate::log!("EFI_STUB: register_protocol_notify\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn locate_handle(
    _: LocateSearchType,
    guid: *mut Guid,
    _: *mut c_void,
    _: *mut usize,
    _: *mut Handle,
) -> Status {
    if guid == core::ptr::null_mut() {
        crate::log!("EFI_STUB: locate_handle - NULL\n");
        return Status::INVALID_PARAMETER;
    }

    crate::log!("EFI_STUB: locate_handle - ");
    print_guid (guid);
    crate::log!("\n");

    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn locate_device_path(_: *mut Guid, _: *mut *mut c_void) -> Status {
    crate::log!("EFI_STUB: locate_device_path\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn install_configuration_table(_: *mut Guid, _: *mut c_void) -> Status {
    crate::log!("EFI_STUB: install_configuration_table\n");

    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn load_image(
    boot_policy: Boolean,
    parent_image_handle: Handle,
    device_path: *mut c_void,
    source_buffer: *mut c_void,
    source_size: usize,
    image_handle: *mut Handle,
) -> Status {
    crate::log!("EFI_STUB: load_image\n");

    let (status, new_image_handle) = IMAGE.lock().load_image(
        source_buffer,
        source_size,
    );

    if status == Status::SUCCESS {
        if image_handle != core::ptr::null_mut() {
          unsafe { *image_handle = new_image_handle };
        };
    }

    status
}

#[cfg(not(test))]
pub extern "win64" fn start_image(
    image_handle: Handle,
    exit_data_size: *mut usize,
    exit_data: *mut *mut Char16
) -> Status {
    crate::log!("EFI_STUB: start_image\n");

    let (status, new_exit_data_size, new_exit_data) = IMAGE.lock().start_image(image_handle);

    if exit_data_size != core::ptr::null_mut() {
      unsafe { *exit_data_size = new_exit_data_size };
    }
    if exit_data != core::ptr::null_mut() {
      unsafe { *exit_data = new_exit_data };
    }

    status
}

#[cfg(not(test))]
pub extern "win64" fn exit(_: Handle, _: Status, _: usize, _: *mut Char16) -> Status {
    crate::log!("EFI_STUB: exit\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn unload_image(_: Handle) -> Status {
    crate::log!("EFI_STUB: unload_image\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn exit_boot_services(_: Handle, _: usize) -> Status {
    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn get_next_monotonic_count(_: *mut u64) -> Status {
    crate::log!("EFI_STUB: get_next_monotonic_count\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn stall(_: usize) -> Status {
    crate::log!("EFI_STUB: stall\n");
    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn set_watchdog_timer(_: usize, _: u64, _: usize, _: *mut Char16) -> Status {
    crate::log!("EFI_STUB: set_watchdog_timer\n");
    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn connect_controller(
    _: Handle,
    _: *mut Handle,
    _: *mut c_void,
    _: Boolean,
) -> Status {
    crate::log!("EFI_STUB: connect_controller\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn disconnect_controller(_: Handle, _: Handle, _: Handle) -> Status {
    crate::log!("EFI_STUB: disconnect_controller\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn open_protocol(
    handle: Handle,
    guid: *mut Guid,
    out_interface: *mut *mut c_void,
    agent_handle: Handle,
    controller_handle: Handle,
    attributes: u32,
) -> Status {
    if unsafe { *guid } == r_efi::protocols::loaded_image::PROTOCOL_GUID {
        unsafe {
            *out_interface = handle;
        }
        return Status::SUCCESS;
    }
    if guid == core::ptr::null_mut() {
        crate::log!("EFI_STUB: open_protocol - NULL\n");
        return Status::INVALID_PARAMETER;
    }

    crate::log!("EFI_STUB: open_protocol - ");
    print_guid (guid);
    crate::log!("\n");

    log!("attributes - {}\n", attributes);

    if attributes != OPEN_PROTOCOL_GET_PROTOCOL {
      return Status::UNSUPPORTED;
    }

    unsafe {*out_interface = core::ptr::null_mut();}
    let (status, interface) = HANDLE_DATABASE.lock().handle_protocol (handle, guid);
    if status == Status::SUCCESS {
      unsafe {*out_interface = interface;}
    }

    status
}

#[cfg(not(test))]
pub extern "win64" fn close_protocol(_: Handle, _: *mut Guid, _: Handle, _: Handle) -> Status {
    crate::log!("EFI_STUB: close_protocol\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn open_protocol_information(
    _: Handle,
    _: *mut Guid,
    _: *mut *mut OpenProtocolInformationEntry,
    _: *mut usize,
) -> Status {
    crate::log!("EFI_STUB: open_protocol_information\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn protocols_per_handle(
    _: Handle,
    _: *mut *mut *mut Guid,
    _: *mut usize,
) -> Status {
    crate::log!("EFI_STUB: protocols_per_handle\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn locate_handle_buffer(
    locate_search_type: LocateSearchType,
    guid: *mut Guid,
    search_key: *mut c_void,
    no_handles: *mut usize,
    buffer: *mut *mut Handle,
) -> Status {
    if guid == core::ptr::null_mut() {
        crate::log!("EFI_STUB: locate_handle_buffer - NULL\n");
        return Status::INVALID_PARAMETER;
    }

    crate::log!("EFI_STUB: locate_handle_buffer - ");
    print_guid (guid);
    crate::log!("\n");

    log!("locate_search_type - {}\n", locate_search_type as u32);
    log!("search_key - {:p}\n", search_key);

    if locate_search_type as u32 != LocateSearchType::ByProtocol as u32 {
      return Status::UNSUPPORTED;
    }
    if search_key != core::ptr::null_mut() {
      return Status::UNSUPPORTED;
    }

    let (status, handle_count, handle_buffer) = HANDLE_DATABASE.lock().locate_handle_buffer(guid);
    if status == Status::SUCCESS {
        unsafe {
            *no_handles = handle_count;
            *buffer = handle_buffer as *mut Handle;
        }
    }
    log!("status - {:?}\n", status);
    status
}

#[cfg(not(test))]
pub extern "win64" fn locate_protocol(guid: *mut Guid, _: *mut c_void, _: *mut *mut c_void) -> Status {
    if guid == core::ptr::null_mut() {
        crate::log!("EFI_STUB: locate_protocol - NULL\n");
        return Status::INVALID_PARAMETER;
    }

    crate::log!("EFI_STUB: locate_protocol - ");
    print_guid (guid);
    crate::log!("\n");

    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn install_multiple_protocol_interfaces(
    handle: *mut Handle,
    guid: *mut c_void,
    interface: *mut c_void,
) -> Status {

    let guid_ptr = guid as *mut Guid;

    crate::log!("EFI_STUB: install_multiple_protocol_interfaces - ");
    print_guid (guid_ptr);
    crate::log!("\n");

    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn uninstall_multiple_protocol_interfaces(
    _: *mut Handle,
    _: *mut c_void,
    _: *mut c_void,
) -> Status {
    crate::log!("EFI_STUB: uninstall_multiple_protocol_interfaces\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn calculate_crc32(_: *mut c_void, _: usize, _: *mut u32) -> Status {
    crate::log!("EFI_STUB: calculate_crc32\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn copy_mem(dest: *mut c_void, source: *mut c_void, size: usize) {
    crate::log!("EFI_STUB: copy_mem\n");
    unsafe {core::ptr::copy (source, dest, size);}
}

#[cfg(not(test))]
pub extern "win64" fn set_mem(buffer: *mut c_void, size: usize, val: u8) {
    crate::log!("EFI_STUB: set_mem\n");
    unsafe {core::ptr::write_bytes (buffer, val, size);}
}

#[cfg(not(test))]
pub extern "win64" fn create_event_ex(
    _: u32,
    _: Tpl,
    _: EventNotify,
    _: *const c_void,
    _: *const Guid,
    event: *mut Event,
) -> Status {
    crate::log!("EFI_STUB: create_event_ex\n");

    if event == core::ptr::null_mut() {
        crate::log!("EFI_STUB: create_event_ex - NULL\n");
        return Status::INVALID_PARAMETER;
    }

    unsafe {*event = core::ptr::null_mut();}

    // TBD
    Status::SUCCESS
}

#[cfg(not(test))]
extern "win64" fn image_unload(_: Handle) -> Status {
    crate::log!("EFI_STUB: image_unload\n");
    efi::Status::UNSUPPORTED
}

#[cfg(not(test))]
pub const PAGE_SIZE: u64 = 4096;

#[cfg(not(test))]
const STDIN_HANDLE: Handle = 0 as Handle;
#[cfg(not(test))]
const STDOUT_HANDLE: Handle = 1 as Handle;
#[cfg(not(test))]
const STDERR_HANDLE: Handle = 2 as Handle;

// HACK: Until r-util/r-efi#11 gets merged
#[cfg(not(test))]
#[repr(C)]
pub struct LoadedImageProtocol {
    pub revision: u32,
    pub parent_handle: Handle,
    pub system_table: *mut efi::SystemTable,

    pub device_handle: Handle,
    pub file_path: *mut r_efi::protocols::device_path::Protocol,
    pub reserved: *mut core::ffi::c_void,

    pub load_options_size: u32,
    pub load_options: *mut core::ffi::c_void,

    pub image_base: *mut core::ffi::c_void,
    pub image_size: u64,
    pub image_code_type: efi::MemoryType,
    pub image_data_type: efi::MemoryType,
    pub unload: eficall! {fn(
        Handle,
    ) -> Status},
}

pub static mut STDIN : SimpleTextInputProtocol = SimpleTextInputProtocol {
          reset: stdin_reset,
          read_key_stroke: stdin_read_key_stroke,
          wait_for_key: 0 as Event,
      };

pub static mut STDOUT_MODE : SimpleTextOutputMode = SimpleTextOutputMode {
        max_mode: 1,
        mode: 0,
        attribute: 0,
        cursor_column: 0,
        cursor_row: 0,
        cursor_visible: Boolean::FALSE,
      };

pub static mut STDOUT : SimpleTextOutputProtocol = SimpleTextOutputProtocol {
        reset: stdout_reset,
        output_string: stdout_output_string,
        test_string: stdout_test_string,
        query_mode: stdout_query_mode,
        set_mode: stdout_set_mode,
        set_attribute: stdout_set_attribute,
        clear_screen: stdout_clear_screen,
        set_cursor_position: stdout_set_cursor_position,
        enable_cursor: stdout_enable_cursor,
        mode: core::ptr::null_mut(),
      };

pub static mut RT : efi::RuntimeServices = efi::RuntimeServices {
        hdr: efi::TableHeader {
            signature: efi::RUNTIME_SERVICES_SIGNATURE,
            revision: efi::RUNTIME_SERVICES_REVISION,
            header_size: core::mem::size_of::<efi::RuntimeServices>() as u32,
            crc32: 0, // TODO
            reserved: 0,
        },
        get_time,
        set_time,
        get_wakeup_time,
        set_wakeup_time,
        set_virtual_address_map,
        convert_pointer,
        get_variable,
        get_next_variable_name,
        set_variable,
        get_next_high_mono_count,
        reset_system,
        update_capsule,
        query_capsule_capabilities,
        query_variable_info,
      };

pub static mut BS : efi::BootServices = efi::BootServices {
        hdr: efi::TableHeader {
            signature: efi::BOOT_SERVICES_SIGNATURE,
            revision: efi::BOOT_SERVICES_REVISION,
            header_size: core::mem::size_of::<efi::BootServices>() as u32,
            crc32: 0, // TODO
            reserved: 0,
        },
        raise_tpl,
        restore_tpl,
        allocate_pages,
        free_pages,
        get_memory_map,
        allocate_pool,
        free_pool,
        create_event,
        set_timer,
        wait_for_event,
        signal_event,
        close_event,
        check_event,
        install_protocol_interface,
        reinstall_protocol_interface,
        uninstall_protocol_interface,
        handle_protocol,
        register_protocol_notify,
        locate_handle,
        locate_device_path,
        install_configuration_table,
        load_image,
        start_image,
        exit,
        unload_image,
        exit_boot_services,
        get_next_monotonic_count,
        stall,
        set_watchdog_timer,
        connect_controller,
        disconnect_controller,
        open_protocol,
        close_protocol,
        open_protocol_information,
        protocols_per_handle,
        locate_handle_buffer,
        locate_protocol,
        install_multiple_protocol_interfaces,
        uninstall_multiple_protocol_interfaces,
        calculate_crc32,
        copy_mem,
        set_mem,
        create_event_ex,
        reserved: core::ptr::null_mut(),
      };

pub static mut CT : efi::ConfigurationTable = efi::ConfigurationTable {
        vendor_guid: Guid::from_fields(0, 0, 0, 0, 0, &[0; 6]), // TODO
        vendor_table: core::ptr::null_mut(),
      };

pub static mut ST : efi::SystemTable = efi::SystemTable {
        hdr: efi::TableHeader {
            signature: efi::SYSTEM_TABLE_SIGNATURE,
            revision: efi::SYSTEM_TABLE_REVISION_2_70,
            header_size: core::mem::size_of::<efi::SystemTable>() as u32,
            crc32: 0, // TODO
            reserved: 0,
        },
        firmware_vendor: core::ptr::null_mut(), // TODO,
        firmware_revision: 0,
        console_in_handle: STDIN_HANDLE,
        con_in: core::ptr::null_mut(),
        console_out_handle: STDOUT_HANDLE,
        con_out: core::ptr::null_mut(),
        standard_error_handle: STDERR_HANDLE,
        std_err: core::ptr::null_mut(),
        runtime_services: core::ptr::null_mut(),
        boot_services: core::ptr::null_mut(),
        number_of_table_entries: 0,
        configuration_table: core::ptr::null_mut(),
      };

#[cfg(not(test))]
pub fn enter_uefi(hob: *const c_void) -> ! {

    unsafe {
      STDOUT.mode = &mut STDOUT_MODE;
      ST.con_in = &mut STDIN;
      ST.con_out = &mut STDOUT;
      ST.std_err = &mut STDOUT;
      ST.runtime_services = &mut RT;
      ST.boot_services = &mut BS;
      ST.configuration_table = &mut CT;
    }
    
    crate::pi::hob_lib::dump_hob (hob);

    crate::efi::init::initialize_memory(hob);
    crate::efi::init::initialize_variable ();

    let (image, size) = crate::efi::init::find_loader (hob);

    let mut image_path = MemoryMappedDevicePathProtocol {
            header: DevicePathProtocol {
                r#type: r_efi::protocols::device_path::TYPE_HARDWARE,
            sub_type: r_efi::protocols::device_path::Hardware::SUBTYPE_MMAP,
            length: [0, 24],
        },
        memory_type: MemoryType::BootServicesCode,
        start_address: image as u64,
        end_address: image as u64 + size as u64 - 1,
    };

    let mut image_handle : Handle = core::ptr::null_mut();
    let status = load_image (
                   Boolean::FALSE,
                   core::ptr::null_mut(), // parent handle
                   &mut image_path.header as *mut DevicePathProtocol as *mut c_void,
                   image as *mut c_void,
                   size,
                   &mut image_handle
                   );
    match (status) {
      Status::SUCCESS => {
        let mut exit_data_size : usize = 0;
        let mut exit_data : *mut Char16 = core::ptr::null_mut();
        let status = start_image (
                       image_handle,
                       &mut exit_data_size as *mut usize,
                       &mut exit_data as *mut *mut Char16
                       );
      },
      _ => {
        log!("load image fails {:?}\n", status);
      },
    }

    log!("Core Init Done\n");
    loop {}
}
