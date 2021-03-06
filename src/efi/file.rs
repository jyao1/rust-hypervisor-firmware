// Copyright © 2019-2020 Intel Corporation
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

use efi_str::OsStr;

use crate::calloc::{free, malloc};
use crate::fat::Filesystem;
use r_efi::efi::{self, Char16, Guid, Status};
use r_efi::protocols::device_path::Protocol as DevicePathProtocol;
use r_efi::protocols::device_path::{HardDriveDevicePath, HardDriveDevicePathNode};
use r_efi::protocols::file::IoToken;
use r_efi::protocols::file::Protocol as FileProtocol;
use r_efi::protocols::simple_file_system::Protocol as SimpleFileSystemProtocol;

use core::ffi::c_void;

use core::fmt;

#[cfg(not(test))]
#[repr(packed)]
struct FileInfo {
    size: u64,
    file_size: u64,
    physical_size: u64,
    _create_time: r_efi::system::Time,
    _last_access_time: r_efi::system::Time,
    _modification_time: r_efi::system::Time,
    attribute: u64,
    file_name: [Char16; crate::fat::EFI_PATH_STRING_LENGTH],
}

pub struct FileSystemWrapper<'a> {
    pub fs: Filesystem<'a>,
    pub proto: SimpleFileSystemProtocol,
}

pub struct FileWrapper<'a> {
    pub fs: &'a Filesystem<'a>,
    pub fs_wrapper: *const FileSystemWrapper<'a>,
    pub proto: FileProtocol,

    pub ofile: crate::fat::OFileDirectory<'a>,
}

impl<'a> FileSystemWrapper<'a> {
    pub unsafe fn new(fs: Filesystem<'a>) -> Result<*mut FileSystemWrapper, Status> {
        let fs_wrapper = malloc::<FileSystemWrapper>()?;

        (*fs_wrapper).fs = fs;
        (*fs_wrapper).proto = SimpleFileSystemProtocol {
            revision: r_efi::protocols::simple_file_system::REVISION,
            open_volume: filesystem_open_volumn,
        };
        Ok(fs_wrapper)
    }

    pub unsafe fn create_file(
        &self,
        ofile: &crate::fat::OFileDirectory<'a>,
    ) -> Result<*mut FileWrapper, Status> {
        let fw = malloc::<FileWrapper>()?;

        (*fw).fs = &(self.fs);

        (*fw).fs_wrapper = self;

        (*fw).proto = FileProtocol {
            revision: r_efi::protocols::file::REVISION,
            open,
            close,
            delete,
            read,
            write,
            get_position,
            set_position,
            get_info,
            set_info,
            flush,
            open_ex,
            read_ex,
            write_ex,
            flush_ex,
        };

        (*fw).ofile = *ofile;

        Ok(fw)
    }

    pub unsafe fn get_hard_drive_device_path(&self) -> Result<*mut c_void, Status> {
        let mut file_system_path = HardDriveDevicePath {
            file_system_path_node: HardDriveDevicePathNode {
                header: DevicePathProtocol {
                    r#type: r_efi::protocols::device_path::TYPE_MEDIA,
                    sub_type: r_efi::protocols::device_path::Hardware::SUBTYPE_PCI,
                    length: [42, 0],
                },
                partition_number: self.fs.part_id,
                partition_start: self.fs.start as u64,
                partition_size: self.fs.last - self.fs.start as u64,
                partition_signature: [0x5452_4150_2049_4645u64, 0],
                partition_format: 0x2 as u8,
                signature_type: 0x2 as u8,
            },
            end: r_efi::protocols::device_path::End {
                header: DevicePathProtocol {
                    r#type: r_efi::protocols::device_path::TYPE_END,
                    sub_type: r_efi::protocols::device_path::End::SUBTYPE_ENTIRE,
                    length: [4, 0],
                },
            },
        };
        let mut fp = crate::calloc::duplicate(&file_system_path)?;
        Ok(fp as *mut c_void)
    }
}

impl core::fmt::Debug for FileInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe {
            write!(
                f,
                "size: {}, file_size: {}, filename: {}, attr: {:x}",
                self.size,
                self.file_size,
                OsStr::from_char16_with_nul(&self.file_name[..] as *const [u16] as *const u16),
                self.attribute
            )
        }
    }
}

impl FileInfo {
    pub fn init(&mut self, de: crate::fat::DirectoryEntry, max_size: usize) -> Result<usize, usize> {
        let mut size = 0usize;

        let mut long_name = de.long_name;
        if long_name[0] == 0 {
            for i in 0..(crate::fat::EFI_FAT_SHORT_NAME_LEN + 1) {
                long_name[i] = de.name[i] as u16;
            }
        }
        let filename = OsStr::from_u16_slice_with_nul(&long_name[..]);
        let mut name_len = filename.len();
        size = (core::mem::size_of::<FileInfo>() - crate::fat::EFI_PATH_STRING_LENGTH * core::mem::size_of::<Char16>() + (name_len + 1) * core::mem::size_of::<Char16>()) as usize;
        if (max_size < size) {
            return Err(size);
        }
        for i in 0..name_len {
            self.file_name[i] = long_name[i];
        }
        self.file_name[name_len] = 0;
        self.attribute = de.attr as u64;
        match de.file_type {
            crate::fat::FileType::File => {
                self.size = size as u64;
                self.file_size = de.size.into();
                self.physical_size = de.size.into();
            }
            crate::fat::FileType::Directory => {
                self.size = size as u64;
                self.file_size = 4096;
                self.physical_size = 4096;
            }
        }
        Ok(size)
    }
}

pub extern "win64" fn filesystem_open_volumn(
    proto: *mut SimpleFileSystemProtocol,
    file: *mut *mut FileProtocol,
) -> Status {
    unsafe {
        let wrapper = container_of!(proto, FileSystemWrapper, proto);
        let wrapper: &FileSystemWrapper = &*wrapper;

        if wrapper.fs.root().is_err() {
            return Status::DEVICE_ERROR;
        }
        let res = wrapper.create_file(&wrapper.fs.root().unwrap());
        match res {
            Err(err) => {
                return err;
            }
            Ok(fw) => {
                *file = &mut (*fw).proto;
                //log!("open_volumn - status: {:x} - path: \\, file: {:x}", Status::SUCCESS.value(), *file as u64);
                return Status::SUCCESS;
            }
        }
    }
}

pub extern "win64" fn open(
    file_in: *mut FileProtocol,
    file_out: *mut *mut FileProtocol,
    path_in: *mut Char16,
    _: u64,
    _: u64,
) -> Status {
    unsafe {
        let wrapper = container_of!(file_in, FileWrapper, proto);
        let wrapper: &FileWrapper = unsafe { &*wrapper };
        let fs_wrapper = unsafe { &(*wrapper.fs_wrapper) };

        let mut path = [0u8;256];
        let mut path_os = OsStr::from_char16_with_nul_mut(path_in);
        let path_len = efi_str::encoder::decode(path_os.as_u16_slice(), &mut path).unwrap_or(0);
        let path = core::str::from_utf8(&(path[0..path_len])).unwrap();

        let mut status = Status::SUCCESS;
        let mut file_out_wrapper: Result<*mut FileWrapper, Status> = Err(Status::SUCCESS);

        if path == ".."
            && wrapper.ofile.get_parent_dir_ent().is_err()
        {
            file_out_wrapper = Err(Status::INVALID_PARAMETER);
        } else {
            match wrapper.fs.open(&wrapper.ofile, &path[..]) {
                Ok(f) => {
                    file_out_wrapper = fs_wrapper.create_file(&f);
                }
                Err(crate::fat::Error::NotFound) => {
                    file_out_wrapper = Err(Status::NOT_FOUND);
                }
                Err(_) => {
                    file_out_wrapper = Err(Status::DEVICE_ERROR);
                }
            }
        };

        match file_out_wrapper {
            Ok(out) => {
                *file_out = &mut (*out).proto;
                status = Status::SUCCESS;
            }
            Err(err) => {
                status = err;
            }
        }

        // log!(
        //     "FSOpen: Status: {:x} PATH: {} IN_FILE: {:x}, OUT_FILE: {:x}\n",
        //     status.value(),
        //     path_os,
        //     file_in as u64,
        //     *file_out as u64
        // );
        status
    }
}

pub extern "win64" fn close(proto: *mut FileProtocol) -> Status {
    // log!("file close: {:x}", proto as u64);
    let wrapper = container_of!(proto, FileWrapper, proto);
    unsafe {
        crate::calloc::free(wrapper as *mut FileWrapper);
    }
    Status::SUCCESS
}

pub extern "win64" fn delete(_: *mut FileProtocol) -> Status {
    // crate::log!("delete unsupported");
    Status::UNSUPPORTED
}

pub fn read_file(file: &mut crate::fat::File, buf: &mut [u8]) -> Result<usize, Status> {
    let mut bytes_remaining = buf.len();
    let mut current_offset = 0;
    use crate::fat::Read;
    let mut status = Status::SUCCESS;
    while bytes_remaining > 0 {
        let mut data: [u8; 512] = [0; 512];
        match file.read(&mut data) {
            Ok(bytes_read) => {
                let mut can_copy_bytes = bytes_read as usize;
                can_copy_bytes = bytes_read as usize;

                if can_copy_bytes > bytes_remaining {
                    can_copy_bytes = bytes_remaining;
                }
                buf[current_offset..current_offset+can_copy_bytes].copy_from_slice(&data[0..can_copy_bytes]);
                bytes_remaining -= can_copy_bytes;
                current_offset += can_copy_bytes;
            }
            Err(crate::fat::Error::EndOfFile) => {
                if current_offset == 0 {
                    status = Status::END_OF_FILE;
                } else {
                    status = Status::SUCCESS;
                }
                break;
            }
            Err(_) => {
                status = Status::DEVICE_ERROR;
            }
        }
    }

    if status.is_error() {
        return Err(status);
    }
    Ok(current_offset)
}

pub extern "win64" fn read(file: *mut FileProtocol, size: *mut usize, buf: *mut c_void) -> Status {
    unsafe {
        // log!("read called {:?} {:?}", file, size);
        let old_size = *size;
        let wrapper = container_of_mut!(file, FileWrapper, proto);
        let wrapper: &mut FileWrapper = &mut (*wrapper);
        let mut status = Status::SUCCESS;
        match wrapper.ofile.dir_ent.file_type {
            crate::fat::FileType::File => {
                let mut file = wrapper.ofile.file.as_mut().unwrap();
                let mut buf = core::slice::from_raw_parts_mut(buf as *mut u8, *size);
                let res = read_file(file, &mut buf);
                match res {
                    Ok(bytes_read) => {
                        status = Status::SUCCESS;
                        *size = bytes_read;
                    }
                    Err(err) => {
                        *size = 0;
                        status = err;
                    }
                }
            }
            crate::fat::FileType::Directory => {
                let info = buf as *mut FileInfo;
                let mut directory = wrapper.ofile.dir.as_mut().unwrap();
                match directory.next_entry() {
                    Err(crate::fat::Error::EndOfFile) => {
                        (*info).size = 0;
                        (*info).attribute = 0;
                        (*info).file_name[0] = 0;
                        (*size) = 0;
                        status = Status::SUCCESS;
                    }
                    Err(e) => {
                        status = Status::DEVICE_ERROR;
                    }
                    Ok(de) => {
                        let ret = (*info).init(de, *size);
                        match ret {
                            Ok(total_size) => { *size = total_size; status = Status::SUCCESS;}
                            Err(need_size) => { *size = need_size; status = Status::BUFFER_TOO_SMALL;}
                        }
                    }
                }
            }
        }
        log!(
            "FRead: Status: {:X}, FILE: {:X}, size: {}, new_size: {:?}",
            status.value(),
            file as u64,
            old_size,
            *size
        );
        status
    }
}

pub extern "win64" fn write(_: *mut FileProtocol, _: *mut usize, _: *mut c_void) -> Status {
    crate::log!("write unsupported");
    Status::UNSUPPORTED
}

pub extern "win64" fn get_position(_: *mut FileProtocol, _: *mut u64) -> Status {
    crate::log!("get_position unsupported");
    Status::UNSUPPORTED
}

pub extern "win64" fn set_position(_file_in: *mut FileProtocol, _pos: u64) -> Status {
    // TODO: set position for opened file and opend directory.
    // crate::log!("set_position: file {:X}, pos {}", file_in as u64, pos);
    Status::SUCCESS
}

pub extern "win64" fn get_info(
    file: *mut FileProtocol,
    guid: *mut Guid,
    info_size: *mut usize,
    info: *mut c_void,
) -> Status {
    unsafe {
        let wrapper = container_of!(file, FileWrapper, proto);
        let wrapper = &(*wrapper);
        if *guid == r_efi::protocols::file::INFO_ID {
            if *info_size < core::mem::size_of::<FileInfo>() {
                *info_size = core::mem::size_of::<FileInfo>();
                Status::BUFFER_TOO_SMALL
            } else {
                let mut status;
                let info = info as *mut FileInfo;
                let ret = (*info).init(wrapper.ofile.dir_ent, *info_size);
                match ret {
                    Ok(total_size) => { *info_size = total_size; status = Status::SUCCESS;}
                    Err(need_size) => { *info_size = need_size; status = Status::BUFFER_TOO_SMALL;}
                }
                log!("FGetInfo: file_in: {:x}, {:?}", file as u64, &*info);
                status
            }
        } else {
            crate::log!("get_info unsupported");
            Status::UNSUPPORTED
        }
    }
}

pub extern "win64" fn set_info(
    _: *mut FileProtocol,
    _: *mut Guid,
    _: usize,
    _: *mut c_void,
) -> Status {
    crate::log!("set_info unsupported");
    Status::UNSUPPORTED
}

pub extern "win64" fn flush(_: *mut FileProtocol) -> Status {
    crate::log!("flush unsupported");
    Status::UNSUPPORTED
}

pub extern "win64" fn open_ex(
    _: *mut FileProtocol,
    _: *mut *mut FileProtocol,
    _: *mut Char16,
    _: u64,
    _: u64,
    _: *mut IoToken,
) -> Status {
    Status::UNSUPPORTED
}

pub extern "win64" fn read_ex(_: *mut FileProtocol, _: *mut IoToken) -> Status {
    Status::UNSUPPORTED
}

pub extern "win64" fn write_ex(_: *mut FileProtocol, _: *mut IoToken) -> Status {
    Status::UNSUPPORTED
}

pub extern "win64" fn flush_ex(_: *mut FileProtocol, _: *mut IoToken) -> Status {
    Status::UNSUPPORTED
}
