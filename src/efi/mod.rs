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
mod event;
mod handle_database;
mod variable;
mod conout;
mod conin;
mod peloader;
mod init;

use lazy_static::lazy_static;
use spin::Mutex;
use core::fmt;
use cpuio::Port;
use core::mem::transmute;

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
use r_efi::protocols::device_path::End as EndDevicePath;

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
use event::EventInfo;
use conout::ConOut;
use conin::ConIn;

#[cfg(not(test))]
#[repr(C,packed)]
pub struct FullMemoryMappedDevicePath {
  memory_map : MemoryMappedDevicePathProtocol,
  end: EndDevicePath,
}

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

lazy_static! {
    pub static ref EVENT: Mutex<EventInfo> = Mutex::new(EventInfo::new());
}

lazy_static! {
    pub static ref CONOUT: Mutex<ConOut> = Mutex::new(ConOut::new());
}

lazy_static! {
    pub static ref CONIN: Mutex<ConIn> = Mutex::new(ConIn::new());
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

pub fn get_char16_size (
    message: *mut Char16,
    max_size: usize
    ) -> usize
{
    let mut i: usize = 0;
    loop {
        if (i >= max_size) {
            break;
        }
        let output = (unsafe { *message.add(i) } & 0xffu16) as u8;
        i += 1;
        if output == 0 {
            break;
        }
    }
    return i
}

pub fn char16_to_char8 (
    in_message: *mut Char16,
    in_message_size: usize,
    out_message: *mut u8,
    out_message_size: usize,
    ) -> usize
{
    let mut i: usize = 0;
    loop {
        if (i >= in_message_size) {
            break;
        }
        if (i >= out_message_size) {
            break;
        }
        let output = (unsafe { *in_message.add(i) } & 0xffu16) as u8;
        unsafe { *out_message.add(i) = output; }
        i += 1;
        if output == 0 {
            break;
        }
    }
    return i;
}

pub fn print_char16 (
    message: *mut Char16,
    max_size: usize
    ) -> usize
{
    let mut i: usize = 0;
    loop {
        if (i >= max_size) {
            break;
        }
        let output = (unsafe { *message.add(i) } & 0xffu16) as u8;
        i += 1;
        if output == 0 {
            break;
        } else {
            crate::log!("{}", output as char);
        }
    }
    return i;
}

#[cfg(not(test))]
pub extern "win64" fn stdin_reset(_: *mut SimpleTextInputProtocol, _: Boolean) -> Status {
    crate::log!("EFI_STUB: stdin_reset\n");
    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn stdin_read_key_stroke(
    _: *mut SimpleTextInputProtocol,
    key: *mut InputKey,
) -> Status {
    crate::log!("EFI_STUB: stdin_read_key_stroke\n");
    let byte = CONIN.lock().read_byte();

    let mut string : [Char16; 8] = ['r' as Char16, 'e' as Char16, 'a' as Char16, 'd' as Char16, 0, 0, '\r' as Char16, 0];

    //crate::log!("read - 0x{:x}\n", byte);

    if byte == 0 {
      unsafe {
        (*key).scan_code = 0;
        (*key).unicode_char = 0;
      }
      return Status::NOT_READY;
    }

    if false {
        let c = (byte >> 4) & 0xF;
        if (c >= 0xa) {
          string[4] = (c - 0xau8 + 'a' as u8) as u16;
        } else {
          string[4] = (c + '0' as u8) as u16;
        }
        let c = byte & 0xF;
        if (c >= 0xa) {
          string[5] = (c - 0xau8 + 'a' as u8) as u16;
        } else {
          string[5] = (c + '0' as u8) as u16;
        }
        stdout_output_string (unsafe {&mut STDOUT}, &mut string as *mut [Char16; 8] as *mut u16);
    }

    unsafe {
      (*key).scan_code = 0;
      (*key).unicode_char = byte as Char16;
    }

    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn stdout_reset(_: *mut SimpleTextOutputProtocol, _: Boolean) -> Status {
    crate::log!("EFI_STUB: stdout_reset\n");
    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn stdout_output_string(
    _: *mut SimpleTextOutputProtocol,
    message: *mut Char16,
) -> Status {

    CONOUT.lock().output_string(message);

    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn stdout_test_string(
    _: *mut SimpleTextOutputProtocol,
    message: *mut Char16,
) -> Status {
    crate::log!("EFI_STUB: stdout_test_string\n");
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
    if columns == core::ptr::null_mut() || raws == core::ptr::null_mut() {
      return Status::INVALID_PARAMETER;
    }
    match mode_number {
      0 => {
        unsafe {
        *columns = 80;
        *raws = 25;
        }
      },
      1 => {
        unsafe {
        *columns = 80;
        *raws = 50;
        }
      },
      _ => { return Status::UNSUPPORTED; },
    }
    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn stdout_set_mode(_: *mut SimpleTextOutputProtocol, mode_number: usize) -> Status {
    crate::log!("EFI_STUB: stdout_set_mode\n");
    CONOUT.lock().set_mode(mode_number)
}

#[cfg(not(test))]
pub extern "win64" fn stdout_set_attribute(_: *mut SimpleTextOutputProtocol, attribute: usize) -> Status {
    crate::log!("EFI_STUB: stdout_set_attribute 0x{:x}\n", attribute);
    CONOUT.lock().set_attribute(attribute);
    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn stdout_clear_screen(_: *mut SimpleTextOutputProtocol) -> Status {
    crate::log!("EFI_STUB: stdout_clear_screen\n");
    CONOUT.lock().clear_screen();
    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn stdout_set_cursor_position(
    _: *mut SimpleTextOutputProtocol,
    column: usize,
    row: usize,
) -> Status {
    crate::log!("EFI_STUB: stdout_set_cursor_position {} {}\n", column, row);
    CONOUT.lock().set_cursor_position(column, row);
    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn stdout_enable_cursor(_: *mut SimpleTextOutputProtocol, visible: Boolean) -> Status {
    crate::log!("EFI_STUB: stdout_enable_cursor\n");
    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn get_time(_: *mut Time, _: *mut TimeCapabilities) -> Status {
    crate::log!("EFI_STUB: get_time - UNSUPPORTED\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn set_time(_: *mut Time) -> Status {
    crate::log!("EFI_STUB: set_time - UNSUPPORTED\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn get_wakeup_time(_: *mut Boolean, _: *mut Boolean, _: *mut Time) -> Status {
    crate::log!("EFI_STUB: get_wakeup_time - UNSUPPORTED\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn set_wakeup_time(_: Boolean, _: *mut Time) -> Status {
    crate::log!("EFI_STUB: set_wakeup_time - UNSUPPORTED\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn set_virtual_address_map(
    map_size: usize,
    descriptor_size: usize,
    version: u32,
    descriptors: *mut MemoryDescriptor,
) -> Status {
    crate::log!("EFI_STUB: set_virtual_address_map - ???\n");
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
    crate::log!("EFI_STUB: convert_pointer - UNSUPPORTED\n");
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

    let var_name_size = get_char16_size (var_name, core::usize::MAX);
    print_char16 (var_name, var_name_size);
    crate::log!(" ");
    print_guid (var_guid);
    crate::log!("\n");

    if var_name_size > MAX_VARIABLE_NAME {
      crate::log!("name too long\n");
      return Status::UNSUPPORTED;
    }

    let mut name_buffer: [u8; MAX_VARIABLE_NAME] = [0; MAX_VARIABLE_NAME];
    char16_to_char8 (
        var_name,
        var_name_size,
        &mut name_buffer as *mut [u8; MAX_VARIABLE_NAME] as *mut u8,
        MAX_VARIABLE_NAME);

    let guid_buffer : *mut [u8; 16] = unsafe { core::mem::transmute::<*mut Guid, *mut [u8; 16]>(var_guid) };

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
    crate::log!("EFI_STUB: get_next_variable - UNSUPPORTED\n");
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
    crate::log!("EFI_STUB: set_variable ");

    let var_name_size = get_char16_size (var_name, core::usize::MAX);
    print_char16 (var_name, var_name_size);
    crate::log!(" ");
    print_guid (var_guid);
    crate::log!("\n");

    if var_name_size > MAX_VARIABLE_NAME {
      crate::log!("name too long\n");
      return Status::UNSUPPORTED;
    }

    let mut name_buffer: [u8; MAX_VARIABLE_NAME] = [0; MAX_VARIABLE_NAME];
    char16_to_char8 (
        var_name,
        var_name_size,
        &mut name_buffer as *mut [u8; MAX_VARIABLE_NAME] as *mut u8,
        MAX_VARIABLE_NAME);

    let guid_buffer : *mut [u8; 16] = unsafe { core::mem::transmute::<*mut Guid, *mut [u8; 16]>(var_guid) };

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
    crate::log!("EFI_STUB: get_next_high_mono_count - UNSUPPORTED\n");
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
    crate::log!("EFI_STUB: update_capsule - UNSUPPORTED\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn query_capsule_capabilities(
    _: *mut *mut CapsuleHeader,
    _: usize,
    _: *mut u64,
    _: *mut ResetType,
) -> Status {
    crate::log!("EFI_STUB: query_capsule_capabilities - UNSUPPORTED\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn query_variable_info(_: u32, _: *mut u64, _: *mut u64, _: *mut u64) -> Status {
    crate::log!("EFI_STUB: query_variable_info - UNSUPPORTED\n");
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
    r#type: u32,
    notify_tpl: Tpl,
    notify_function: EventNotify,
    notify_context: *mut c_void,
    event: *mut Event,
) -> Status {
    crate::log!("EFI_STUB: create_event - type:0x{:x} tpl:0x{:x}\n", r#type, notify_tpl as usize);

    let (status, new_event) = EVENT.lock().create_event(
            r#type,
            notify_tpl,
            notify_function,
            notify_context
            );
    log!("status - {:?}\n", status);
    if status == Status::SUCCESS {
        unsafe {
            *event = new_event;
        }
    }
    status
}

#[cfg(not(test))]
pub extern "win64" fn set_timer(_: Event, _: TimerDelay, _: u64) -> Status {
    crate::log!("EFI_STUB: set_timer - UNSUPPORTED\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn wait_for_event(_: usize, _: *mut Event, _: *mut usize) -> Status {
    crate::log!("EFI_STUB: wait_for_event - UNSUPPORTED\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn signal_event(_: Event) -> Status {
    crate::log!("EFI_STUB: signal_event - UNSUPPORTED\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn close_event(_: Event) -> Status {
    crate::log!("EFI_STUB: close_event - UNSUPPORTED\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn check_event(_: Event) -> Status {
    crate::log!("EFI_STUB: check_event - UNSUPPORTED\n");
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
    crate::log!("EFI_STUB: reinstall_protocol_interface - UNSUPPORTED\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn uninstall_protocol_interface(
    _: Handle,
    _: *mut Guid,
    _: *mut c_void,
) -> Status {
    crate::log!("EFI_STUB: uninstall_protocol_interface - UNSUPPORTED\n");
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
    crate::log!("EFI_STUB: register_protocol_notify - UNSUPPORTED\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn locate_handle(
    locate_search_type: LocateSearchType,
    guid: *mut Guid,
    search_key: *mut c_void,
    buffer_size: *mut usize,
    buffer: *mut Handle,
) -> Status {
    if guid == core::ptr::null_mut() {
        crate::log!("EFI_STUB: locate_handle - NULL\n");
        return Status::INVALID_PARAMETER;
    }

    crate::log!("EFI_STUB: locate_handle - ");
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

    let input_buffer_size = unsafe { *buffer_size };
    let (status, final_buffer_size) = HANDLE_DATABASE.lock().locate_handle(guid, input_buffer_size, buffer);
    match status {
      Status::SUCCESS => {},
      Status::BUFFER_TOO_SMALL => {},
      _ => {return status;}
    }

    unsafe { *buffer_size = final_buffer_size; }

    status
}

#[cfg(not(test))]
pub extern "win64" fn locate_device_path(_: *mut Guid, _: *mut *mut c_void) -> Status {
    crate::log!("EFI_STUB: locate_device_path - UNSUPPORTED\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn install_configuration_table(_: *mut Guid, _: *mut c_void) -> Status {
    crate::log!("EFI_STUB: install_configuration_table - UNSUPPORTED\n");

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
        parent_image_handle,
        device_path,
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
    crate::log!("EFI_STUB: exit - UNSUPPORTED\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn unload_image(_: Handle) -> Status {
    crate::log!("EFI_STUB: unload_image - UNSUPPORTED\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn exit_boot_services(_: Handle, _: usize) -> Status {
    crate::log!("EFI_STUB: exit_boot_services\n");
    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn get_next_monotonic_count(_: *mut u64) -> Status {
    crate::log!("EFI_STUB: get_next_monotonic_count - UNSUPPORTED\n");
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
    crate::log!("EFI_STUB: connect_controller - UNSUPPORTED\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn disconnect_controller(_: Handle, _: Handle, _: Handle) -> Status {
    crate::log!("EFI_STUB: disconnect_controller - UNSUPPORTED\n");
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
    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn open_protocol_information(
    _: Handle,
    _: *mut Guid,
    _: *mut *mut OpenProtocolInformationEntry,
    _: *mut usize,
) -> Status {
    crate::log!("EFI_STUB: open_protocol_information - UNSUPPORTED\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn protocols_per_handle(
    _: Handle,
    _: *mut *mut *mut Guid,
    _: *mut usize,
) -> Status {
    crate::log!("EFI_STUB: protocols_per_handle - UNSUPPORTED\n");
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
pub extern "win64" fn locate_protocol(guid: *mut Guid, registration: *mut c_void, interface: *mut *mut c_void) -> Status {
    if guid == core::ptr::null_mut() {
        crate::log!("EFI_STUB: locate_protocol - NULL\n");
        return Status::INVALID_PARAMETER;
    }

    crate::log!("EFI_STUB: locate_protocol - ");
    print_guid (guid);
    crate::log!("\n");

    let (status, new_interface) = HANDLE_DATABASE.lock().locate_protocol(guid);
    if status == Status::SUCCESS {
      unsafe {*interface = new_interface; }
    }

    log!("status - {:?}\n", status);
    status
}
//
// NOTE:
// see https://github.com/rust-lang/rfcs/blob/master/text/2137-variadic.md
// Current vararg support only "C".
// "win64" is not supported.
//
// As such we cannot use below:
// pub unsafe extern "C" fn install_multiple_protocol_interfaces(
//    handle: *mut Handle,
//    mut args: ...
// ) -> Status;
//
// NOTE: Current EDKII has use case with 5 guid/interface pairs.
// So we hardcode to support 8 pairs as maximum. It should be enought.
//
#[cfg(not(test))]
pub extern "win64" fn install_multiple_protocol_interfaces_real(
    handle: *mut Handle,
    guid1: *mut Guid,
    interface1: *mut c_void,
    guid2: *mut Guid,
    interface2: *mut c_void,
    guid3: *mut Guid,
    interface3: *mut c_void,
    guid4: *mut Guid,
    interface4: *mut c_void,
    guid5: *mut Guid,
    interface5: *mut c_void,
    guid6: *mut Guid,
    interface6: *mut c_void,
    guid7: *mut Guid,
    interface7: *mut c_void,
    guid8: *mut Guid,
    interface8: *mut c_void,
    guid_null: *mut c_void,
) -> Status {
    let mut count : usize = 0;
    let mut pair : [(*mut Guid, *mut c_void); 8] = [(core::ptr::null_mut(), core::ptr::null_mut()) ; 8];

    if guid1 == core::ptr::null_mut() {
      crate::log!("EFI_STUB: install_multiple_protocol_interfaces_real - no GUID/Interface pair\n");
      return Status::INVALID_PARAMETER;
    } else {
      count = 1;
      pair[0] = (guid1, interface1);
    }
    if guid2 != core::ptr::null_mut() {
      count = 2;
      pair[1] = (guid2, interface2);
    }
    if guid3 != core::ptr::null_mut() {
      count = 3;
      pair[2] = (guid3, interface3);
    }
    if guid4 != core::ptr::null_mut() {
      count = 4;
      pair[3] = (guid4, interface4);
    }
    if guid5 != core::ptr::null_mut() {
      count = 5;
      pair[4] = (guid5, interface5);
    }
    if guid6 != core::ptr::null_mut() {
      count = 6;
      pair[5] = (guid6, interface6);
    }
    if guid7 != core::ptr::null_mut() {
      count = 7;
      pair[6] = (guid7, interface7);
    }
    if guid8 != core::ptr::null_mut() {
      count = 8;
      pair[7] = (guid8, interface8);
    }
    if guid_null != core::ptr::null_mut() {
      crate::log!("EFI_STUB: install_multiple_protocol_interfaces_real - too many GUID/Interface pair\n");
      return Status::UNSUPPORTED;
    }

    crate::log!("EFI_STUB: install_multiple_protocol_interfaces_real:\n");
    for index in 0 .. count {
      crate::log!("  ");
      print_guid (pair[index].0);
      crate::log!("  ");
      crate::log!("{:p}", pair[index].1);
      crate::log!("\n");
    }

    let (status, new_handle) = HANDLE_DATABASE.lock().install_multiple_protocol(
                unsafe {*handle},
                count,
                &mut pair
                );
    log!("status - {:?}\n", status);
    if status == Status::SUCCESS {
        unsafe {
            *handle = new_handle;
        }
    }
    status
}

pub extern "win64" fn uninstall_multiple_protocol_interfaces_real(
    handle: *mut Handle,
    guid1: *mut Guid,
    interface1: *mut c_void,
    guid2: *mut Guid,
    interface2: *mut c_void,
    guid3: *mut Guid,
    interface3: *mut c_void,
    guid4: *mut Guid,
    interface4: *mut c_void,
    guid5: *mut Guid,
    interface5: *mut c_void,
    guid6: *mut Guid,
    interface6: *mut c_void,
    guid7: *mut Guid,
    interface7: *mut c_void,
    guid8: *mut Guid,
    interface8: *mut c_void,
    guid_null: *mut c_void,
) -> Status {
    crate::log!("EFI_STUB: uninstall_multiple_protocol_interfaces_real - UNSUPPORTED\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn install_multiple_protocol_interfaces(
    handle: *mut Handle,
    guid: *mut c_void,
    interface: *mut c_void,
) -> Status {

    let guid_ptr = guid as *mut Guid;

    crate::log!("EFI_STUB: install_multiple_protocol_interfaces - UNSUPPORTED - ");
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
    crate::log!("EFI_STUB: uninstall_multiple_protocol_interfaces - UNSUPPORTED\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn calculate_crc32(_: *mut c_void, _: usize, _: *mut u32) -> Status {
    crate::log!("EFI_STUB: calculate_crc32\n");
    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn copy_mem(dest: *mut c_void, source: *mut c_void, size: usize) {
    unsafe {core::ptr::copy (source, dest, size);}
}

#[cfg(not(test))]
pub extern "win64" fn set_mem(buffer: *mut c_void, size: usize, val: u8) {
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
    crate::log!("EFI_STUB: create_event_ex - UNSUPPORTED\n");

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
    crate::log!("EFI_STUB: image_unload - UNSUPPORTED\n");
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

pub type InstallMultipleProtocolInterfacesFunc = extern "win64" fn(*mut Handle, *mut c_void, *mut c_void) -> r_efi::base::Status;
pub type UninstallMultipleProtocolInterfacesFunc = extern "win64" fn(*mut Handle,*mut c_void,*mut c_void,) -> r_efi::base::Status;

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

const MAX_CONFIGURATION_TABLE : usize = 4;

pub static mut CT : [efi::ConfigurationTable; MAX_CONFIGURATION_TABLE] = 
        [
          efi::ConfigurationTable {
            vendor_guid: Guid::from_fields(0, 0, 0, 0, 0, &[0; 6]), // TODO
            vendor_table: core::ptr::null_mut(),},
          efi::ConfigurationTable {
            vendor_guid: Guid::from_fields(0, 0, 0, 0, 0, &[0; 6]), // TODO
            vendor_table: core::ptr::null_mut(),},
          efi::ConfigurationTable {
            vendor_guid: Guid::from_fields(0, 0, 0, 0, 0, &[0; 6]), // TODO
            vendor_table: core::ptr::null_mut(),},
          efi::ConfigurationTable {
            vendor_guid: Guid::from_fields(0, 0, 0, 0, 0, &[0; 6]), // TODO
            vendor_table: core::ptr::null_mut(),},
        ];

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

      let func_addr_ptr = unsafe {transmute::<&mut InstallMultipleProtocolInterfacesFunc, *mut usize>(&mut BS.install_multiple_protocol_interfaces)};
      unsafe {*func_addr_ptr = install_multiple_protocol_interfaces_real as usize;}
      let func_addr_ptr = unsafe {transmute::<&mut UninstallMultipleProtocolInterfacesFunc, *mut usize>(&mut BS.uninstall_multiple_protocol_interfaces)};
      unsafe {*func_addr_ptr = uninstall_multiple_protocol_interfaces_real as usize;}

      ST.number_of_table_entries = MAX_CONFIGURATION_TABLE;
      ST.configuration_table = &mut CT as *mut [r_efi::system::ConfigurationTable; MAX_CONFIGURATION_TABLE] as *mut r_efi::system::ConfigurationTable;
    }
    
    crate::pi::hob_lib::dump_hob (hob);

    crate::efi::init::initialize_memory(hob);
    let new_hob = crate::pi::hob_lib::relocate_hob (hob);
    unsafe {
      CT[0].vendor_guid = crate::pi::hob::HOB_LIST_GUID;
      CT[0].vendor_table = new_hob;
    }

    crate::efi::init::initialize_variable ();

    let (image, size) = crate::efi::init::find_loader (new_hob);

    let mut image_path = FullMemoryMappedDevicePath {
        memory_map: MemoryMappedDevicePathProtocol {
            header: DevicePathProtocol {
            r#type: r_efi::protocols::device_path::TYPE_HARDWARE,
            sub_type: r_efi::protocols::device_path::Hardware::SUBTYPE_MMAP,
            length: [24, 0],
          },
          memory_type: MemoryType::BootServicesCode,
          start_address: image as u64,
          end_address: image as u64 + size as u64 - 1,
        },
        end: r_efi::protocols::device_path::End {
            header: DevicePathProtocol {
            r#type: r_efi::protocols::device_path::TYPE_END,
            sub_type: r_efi::protocols::device_path::End::SUBTYPE_ENTIRE,
            length: [24, 0],
          },
        },
    };

    let mut image_handle : Handle = core::ptr::null_mut();
    let status = load_image (
                   Boolean::FALSE,
                   core::ptr::null_mut(), // parent handle
                   &mut image_path.memory_map.header as *mut DevicePathProtocol as *mut c_void,
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
