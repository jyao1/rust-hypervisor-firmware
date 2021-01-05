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

use core::ffi::c_void;

use r_efi::efi::{AllocateType, MemoryType, Status};
use r_efi::protocols::device_path::Protocol as DevicePathProtocol;
use r_efi::{eficall, eficall_abi};

#[cfg(not(test))]
#[repr(packed)]
pub struct HardDiskDevicePathProtocol {
    pub device_path: DevicePathProtocol,
    pub partition_number: u32,
    pub partition_start: u64,
    pub partition_size: u64,
    pub partition_signature: [u8; 16],
    pub partition_format: u8,
    pub signature_type: u8,
}

#[cfg(not(test))]
#[repr(C)]
pub struct BlockIoMedia {
    media_id: u32,
    removable_media: bool,
    media_present: bool,
    logical_partition: bool,
    read_only: bool,
    write_caching: bool,
    block_size: u32,
    io_align: u32,
    last_block: u64,
}

#[cfg(not(test))]
#[repr(C)]
pub struct BlockIoProtocol {
    revision: u64,
    media: *const BlockIoMedia,
    reset: eficall! {fn(
        *mut BlockIoProtocol,
        bool
    ) -> Status},
    read_blocks: eficall! {fn(
        *mut BlockIoProtocol,
        u32,
        u64,
        usize,
        *mut c_void
    ) -> Status},
    write_blocks: eficall! {fn(
        *mut BlockIoProtocol,
        u32,
        u64,
        usize,
        *mut c_void
    ) -> Status},
    flush_blocks: eficall! {fn(
        *mut BlockIoProtocol,
    ) -> Status},
}

#[cfg(not(test))]
pub extern "win64" fn reset(_: *mut BlockIoProtocol, _: bool) -> Status {
    crate::log!("reset unsupported");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn read_blocks(
    proto: *mut BlockIoProtocol,
    _: u32,
    start: u64,
    size: usize,
    buffer: *mut c_void,
) -> Status {
    let wrapper = container_of!(proto, BlockWrapper, proto);
    let wrapper = unsafe { &*wrapper };

    let blocks = (size / 512) as usize;
    let mut region = crate::mem::MemoryRegion::new(buffer as u64, size as u64);

    for i in 0..blocks {
        use crate::block::SectorRead;
        let data = region.as_mut_slice(i as u64 * 512, 512);
        let block = unsafe { &*wrapper.block };
        match block.read(wrapper.start_lba + start + i as u64, data) {
            Ok(()) => continue,
            Err(_) => {
                return Status::DEVICE_ERROR;
            }
        };
    }

    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn write_blocks(
    _: *mut BlockIoProtocol,
    _: u32,
    _: u64,
    _: usize,
    _: *mut c_void,
) -> Status {
    crate::log!("write_blocks unsupported");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn flush_blocks(_: *mut BlockIoProtocol) -> Status {
    crate::log!("flush_blocks unsupported");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
#[repr(C)]
pub struct ControllerDevicePathProtocol {
    pub device_path: DevicePathProtocol,
    controller: u32,
}

#[cfg(not(test))]
#[repr(C)]
pub struct BlockWrapper<'a> {
    hw: super::HandleWrapper,
    block: *const crate::block::VirtioBlockDevice<'a>,
    pub media: BlockIoMedia,
    pub proto: BlockIoProtocol,
    // The ordering of these paths are very important, along with the C
    // representation as the device path "flows" from the first.
    pub controller_path: ControllerDevicePathProtocol,
    pub disk_paths: BlockDevicePath,
    start_lba: u64,
}

#[cfg(not(test))]
pub struct BlockWrappers<'a> {
    pub wrappers: [*mut BlockWrapper<'a>; 16],
    pub count: usize,
}


#[cfg(not(test))]
#[repr(C,packed)]
pub struct BlockDevicePath {
    pub pci: r_efi::protocols::device_path::PciDevicePathNode,
    pub hard_disk: HardDiskDevicePathProtocol,
    pub end: DevicePathProtocol
}


#[cfg(not(test))]
impl<'a> BlockWrapper<'a> {
    pub fn new(
        block: *const crate::block::VirtioBlockDevice,
        partition_number: u32,
        start_lba: u64,
        last_lba: u64,
        uuid: [u8; 16],
    ) -> *mut BlockWrapper {
        let last_block = unsafe { (*block).get_capacity() } - 1;

        let size = core::mem::size_of::<BlockWrapper>();
        let (_status, new_address) = super::ALLOCATOR.lock().allocate_pages(
            AllocateType::AllocateAnyPages,
            MemoryType::LoaderData,
            ((size + super::PAGE_SIZE as usize - 1) / super::PAGE_SIZE as usize) as u64,
            0 as u64,
        );

        let bw = new_address as *mut BlockWrapper;

        unsafe {
            *bw = BlockWrapper {
                hw: super::HandleWrapper {
                    handle_type: super::HandleType::Block,
                },
                block,
                media: BlockIoMedia {
                    media_id: 0,
                    removable_media: false,
                    media_present: true,
                    logical_partition: false,
                    read_only: true,
                    write_caching: false,
                    block_size: 512,
                    io_align: 0,
                    last_block,
                },
                proto: BlockIoProtocol {
                    revision: 0x0001_0000, // EFI_BLOCK_IO_PROTOCOL_REVISION
                    media: core::ptr::null(),
                    reset,
                    read_blocks,
                    write_blocks,
                    flush_blocks,
                },
                start_lba,
                controller_path: ControllerDevicePathProtocol {
                    device_path: DevicePathProtocol {
                        r#type: 1,
                        sub_type: 5,
                        length: [8, 0],
                    },
                    controller: 0,
                },
                // full disk vs partition
                disk_paths:
                    BlockDevicePath {
                        pci: r_efi::protocols::device_path::PciDevicePathNode {
                            header: DevicePathProtocol {
                                r#type: r_efi::protocols::device_path::TYPE_HARDWARE,
                                sub_type: 1,
                                length: [6, 0],
                            },
                            function: 0,
                            device: 3, // GPT
                        },
                        hard_disk: HardDiskDevicePathProtocol {
                            device_path: DevicePathProtocol {
                                r#type: r_efi::protocols::device_path::TYPE_MEDIA,
                                sub_type: 1,
                                length: [42, 0],
                            },
                            partition_number,
                            partition_format: 0x02, // GPT
                            partition_start: start_lba,
                            partition_size: last_lba - start_lba + 1,
                            partition_signature: uuid,
                            signature_type: 0x02,
                        },
                        end: DevicePathProtocol {
                            r#type: r_efi::protocols::device_path::TYPE_END,
                            sub_type: 0xff, // End of full path
                            length: [4, 0],
                        }
                    },
            };

            (*bw).proto.media = &(*bw).media;
        }
        bw
    }
}

fn dup_device_path(device_path: *mut c_void) -> *mut core::ffi::c_void{
    let mut device_path_buffer: *mut c_void = core::ptr::null_mut();
    let device_path_size = crate::efi::device_path::get_device_path_size (device_path as *mut DevicePathProtocol);
    let status = crate::efi::allocate_pool (MemoryType::BootServicesData, device_path_size, &mut device_path_buffer);
    unsafe {core::ptr::copy_nonoverlapping (device_path, device_path_buffer, device_path_size);}

    device_path_buffer
}


#[cfg(not(test))]
#[allow(clippy::transmute_ptr_to_ptr)]
pub fn populate_block_wrappers(
    wrappers: &mut BlockWrappers,
    block: *const crate::block::VirtioBlockDevice,
) -> Option<u32> {
    let mut last_lba = 0u64;
    let mut parts: [crate::part::PartitionEntry; 16] = unsafe { core::mem::zeroed() };

    log!("populate_block_wrappers...\n");

    let mut efi_part_id = Some(0);
    let part_count = crate::part::get_partitions(unsafe { &*block }, &mut parts).unwrap();
    for i in 0..part_count {
        let p = parts[i as usize];
        wrappers.wrappers[i as usize + 1] = BlockWrapper::new(
            unsafe { core::mem::transmute(block) },
            i + 1,
            p.first_lba,
            p.last_lba,
            p.guid,
        );
        if last_lba < p.last_lba {
            last_lba = p.last_lba;
        }
        log!("par {}\n", i);
        if p.is_efi_partition() {
            log!("  is_efi_partition\n");
            efi_part_id = Some(i + 1);
        }
        let mut handle : r_efi::efi::Handle = core::ptr::null_mut();
        let status = crate::efi::install_protocol_interface (
            &mut handle,
            &mut r_efi::protocols::block_io::PROTOCOL_GUID as *mut r_efi::efi::Guid,
            r_efi::efi::InterfaceType::NativeInterface,
            unsafe{&mut ((*wrappers.wrappers[i as usize + 1]).proto)} as *const BlockIoProtocol as *const c_void as *mut c_void
            );
        let status = crate::efi::install_protocol_interface (
            &mut handle,
            &mut r_efi::protocols::device_path::PROTOCOL_GUID as *mut r_efi::efi::Guid,
            r_efi::efi::InterfaceType::NativeInterface,
            unsafe{&mut ((*wrappers.wrappers[i as usize + 1]).disk_paths)} as *const BlockDevicePath as *const c_void as *mut c_void
            );
        // let mut handle : r_efi::efi::Handle = core::ptr::null_mut();
        // let status = crate::efi::install_protocol_interface (
        //     &mut handle,
        //     &mut r_efi::protocols::block_io::PROTOCOL_GUID as *mut r_efi::efi::Guid,
        //     r_efi::efi::InterfaceType::NativeInterface,
        //     unsafe{&mut ((*wrappers.wrappers[i as usize + 1]).proto)} as *const BlockIoProtocol as *const c_void as *mut c_void
        //     );

        // let mut device_path = r_efi::protocols::device_path::PciDevicePath {
        //     pci_device_path: r_efi::protocols::device_path::PciDevicePathNode {
        //         header: DevicePathProtocol {
        //             r#type: r_efi::protocols::device_path::TYPE_HARDWARE,
        //             sub_type: 1,
        //             length: [6, 0],
        //         },
        //         function: 0,
        //         device: 3,
        //     },
        //     end: DevicePathProtocol {
        //         r#type: r_efi::protocols::device_path::TYPE_END,
        //         sub_type: 0xff, // End of full path
        //         length: [4, 0],
        //     }
        // };
        // let device_path_buffer = dup_device_path(&mut device_path.pci_device_path.header as *mut DevicePathProtocol as *mut c_void);
        // let status = crate::efi::install_protocol_interface (
        //     &mut handle,
        //     &mut r_efi::protocols::device_path::PROTOCOL_GUID as *mut r_efi::efi::Guid,
        //     r_efi::efi::InterfaceType::NativeInterface,
        //     device_path_buffer
        //     );
    }

    wrappers.wrappers[0] =
    BlockWrapper::new(unsafe { core::mem::transmute(block) }, 0, 0, last_lba, [0; 16]);
    let mut handle : r_efi::efi::Handle = core::ptr::null_mut();
    let status = crate::efi::install_protocol_interface (
        &mut handle,
        &mut r_efi::protocols::block_io::PROTOCOL_GUID as *mut r_efi::efi::Guid,
        r_efi::efi::InterfaceType::NativeInterface,
        unsafe{&mut ((*wrappers.wrappers[0]).proto)} as *const BlockIoProtocol as *const c_void as *mut c_void
        );
        let mut device_path = r_efi::protocols::device_path::PciDevicePath {
            pci_device_path: r_efi::protocols::device_path::PciDevicePathNode {
                header: DevicePathProtocol {
                    r#type: r_efi::protocols::device_path::TYPE_HARDWARE,
                    sub_type: 1,
                    length: [6, 0],
                },
                function: 0,
                device: 3,
            },
            end: DevicePathProtocol {
                r#type: r_efi::protocols::device_path::TYPE_END,
                sub_type: 0xff, // End of full path
                length: [4, 0],
            }
        };
        let device_path_buffer = dup_device_path(&mut device_path.pci_device_path.header as *mut DevicePathProtocol as *mut c_void);
        let status = crate::efi::install_protocol_interface (
            &mut handle,
            &mut r_efi::protocols::device_path::PROTOCOL_GUID as *mut r_efi::efi::Guid,
            r_efi::efi::InterfaceType::NativeInterface,
            device_path_buffer
            );

    wrappers.count = part_count as usize + 1;
    log!("wrappers.count {}\n", wrappers.count);
    log!("efi_part_id {:?}\n", efi_part_id);
    efi_part_id
}

#[cfg(not(test))]
#[allow(clippy::transmute_ptr_to_ptr)]
pub fn get_block_io_media_str(wrappers: &mut BlockWrappers, index: usize) -> (*mut c_void) {
    unsafe{let ref_1: &mut BlockIoMedia = &mut (*wrappers.wrappers[index]).media;
    return ref_1 as *mut BlockIoMedia as *mut c_void;
    }
}