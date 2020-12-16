// Copyright © 2020 Intel Corporation
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

use r_efi::efi::{AllocateType, Char16, Guid, MemoryType, Status};
use crate::efi::{ALLOCATOR, PAGE_SIZE};
use core::ffi::c_void;

pub fn malloc<T>() -> Result<*mut T, Status> {
    let size = core::mem::size_of::<T>();
    let (status, address) = ALLOCATOR.lock().allocate_pages(
        AllocateType::AllocateAnyPages,
        MemoryType::LoaderData,
        ((size + PAGE_SIZE as usize - 1) / PAGE_SIZE as usize) as u64,
        0 as u64
    );

    if status != Status::SUCCESS {
        Err(Status::OUT_OF_RESOURCES)
    } else {
        Ok(unsafe{core::mem::transmute::<*mut c_void, *mut T>(address as *mut c_void)})
    }
}

pub fn free<T>(ptr: *mut T) {
    ALLOCATOR.lock().free_pages(&ptr as *const _ as u64);
}

pub fn duplicate<T>(d: &T) -> Result<*mut T, Status> {
    let t = malloc::<T>()?;
    unsafe {
        core::ptr::copy_nonoverlapping(
            d as *const T as *const c_void,
            t as *mut c_void,
            core::mem::size_of::<T>(),
        );
    }
    Ok(t)
}
