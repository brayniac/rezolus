use crate::agent::*;
use crate::agent::cgroup::{CgroupInfo, ProcessCgroupEvent};

use std::ffi::CStr;
use std::str;

pub struct CgroupInfo {
    pub id: usize,
    pub name: String,
}

pub trait ProcessCgroupEvent<T> {
    fn set_name(&self, id: usize, name: String);
}

pub fn format_cgroup_path(name: &str, pname: &str, gpname: &str, level: u32) -> String {
    if !gpname.is_empty() {
        if level > 3 {
            format!(".../{gpname}/{pname}/{name}")
        } else {
            format!("/{gpname}/{pname}/{name}")
        }
    } else if !pname.is_empty() {
        format!("/{pname}/{name}")
    } else if !name.is_empty() {
        format!("/{name}")
    } else {
        "".to_string()
    }
}

pub fn handle_cgroup_event<T, H: ProcessCgroupEvent<T>>(handler: &H, data: &[u8]) -> i32 {
    if data.len() < std::mem::size_of::<T>() {
        return 0;
    }

    // Safe to transmute as long as T is a struct that matches the expected layout
    let info = unsafe { &*(data.as_ptr() as *const T) };
    
    // The following fields should be at the same offset in all cgroup_info structs
    let id = unsafe { *(data.as_ptr() as *const u32) };
    let level = unsafe { *(data.as_ptr().add(4) as *const u32) };
    
    // These offsets should match the BPF cgroup_info struct
    let name_offset = 8;
    let name_len = 64;
    let pname_offset = name_offset + name_len;
    let gpname_offset = pname_offset + name_len;
    
    let name = extract_cstr(&data[name_offset..name_offset+name_len]);
    let pname = extract_cstr(&data[pname_offset..pname_offset+name_len]);
    let gpname = extract_cstr(&data[gpname_offset..gpname_offset+name_len]);
    
    let path = format_cgroup_path(&name, &pname, &gpname, level);
    
    handler.set_name(id as usize, path);
    
    0
}

fn extract_cstr(data: &[u8]) -> String {
    match CStr::from_bytes_until_nul(data) {
        Ok(cstr) => cstr.to_str().unwrap_or("").replace("\\x2d", "-"),
        Err(_) => "".to_string(),
    }
}

