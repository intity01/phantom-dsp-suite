#![cfg(windows)]

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::ptr;

const OFN_ALLOWMULTISELECT: u32 = 0x0000_0200;
const OFN_PATHMUSTEXIST: u32 = 0x0000_0800;
const OFN_FILEMUSTEXIST: u32 = 0x0000_1000;
const OFN_OVERWRITEPROMPT: u32 = 0x0000_0002;
const OFN_EXPLORER: u32 = 0x0008_0000;
const OFN_NOCHANGEDIR: u32 = 0x0000_0008;
const BIF_RETURNONLYFSDIRS: u32 = 0x0000_0001;
const BIF_NEWDIALOGSTYLE: u32 = 0x0000_0040;

#[repr(C)]
struct OpenFileNameW {
    l_struct_size: u32,
    hwnd_owner: *mut core::ffi::c_void,
    h_instance: *mut core::ffi::c_void,
    lpstr_filter: *const u16,
    lpstr_custom_filter: *mut u16,
    n_max_cust_filter: u32,
    n_filter_index: u32,
    lpstr_file: *mut u16,
    n_max_file: u32,
    lpstr_file_title: *mut u16,
    n_max_file_title: u32,
    lpstr_initial_dir: *const u16,
    lpstr_title: *const u16,
    flags: u32,
    n_file_offset: u16,
    n_file_extension: u16,
    lpstr_def_ext: *const u16,
    l_cust_data: isize,
    lpfn_hook: *const core::ffi::c_void,
    lp_template_name: *const u16,
    pv_reserved: *mut core::ffi::c_void,
    dw_reserved: u32,
    flags_ex: u32,
}

#[repr(C)]
struct BrowseInfoW {
    hwnd_owner: *mut core::ffi::c_void,
    pidl_root: *const core::ffi::c_void,
    psz_display_name: *mut u16,
    lpsz_title: *const u16,
    ul_flags: u32,
    lpfn: *const core::ffi::c_void,
    l_param: isize,
    i_image: i32,
}

#[link(name = "comdlg32")]
extern "system" {
    fn GetOpenFileNameW(ofn: *mut OpenFileNameW) -> i32;
    fn GetSaveFileNameW(ofn: *mut OpenFileNameW) -> i32;
}

#[link(name = "shell32")]
extern "system" {
    fn SHBrowseForFolderW(lpbi: *mut BrowseInfoW) -> *mut core::ffi::c_void;
    fn SHGetPathFromIDListW(pidl: *const core::ffi::c_void, psz_path: *mut u16) -> i32;
}

#[link(name = "ole32")]
extern "system" {
    fn CoTaskMemFree(pv: *mut core::ffi::c_void);
}

fn to_wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn open_files(title: &str, filter: &str, multi: bool) -> Option<Vec<PathBuf>> {
    let mut buf = vec![0u16; 16384];
    let title_w = to_wide_null(title);
    let filter_w: Vec<u16> = filter.encode_utf16().collect();
    let mut flags = OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
    if multi {
        flags |= OFN_ALLOWMULTISELECT;
    }
    let mut ofn = OpenFileNameW {
        l_struct_size: std::mem::size_of::<OpenFileNameW>() as u32,
        hwnd_owner: ptr::null_mut(),
        h_instance: ptr::null_mut(),
        lpstr_filter: filter_w.as_ptr(),
        lpstr_custom_filter: ptr::null_mut(),
        n_max_cust_filter: 0,
        n_filter_index: 1,
        lpstr_file: buf.as_mut_ptr(),
        n_max_file: buf.len() as u32,
        lpstr_file_title: ptr::null_mut(),
        n_max_file_title: 0,
        lpstr_initial_dir: ptr::null(),
        lpstr_title: title_w.as_ptr(),
        flags,
        n_file_offset: 0,
        n_file_extension: 0,
        lpstr_def_ext: ptr::null(),
        l_cust_data: 0,
        lpfn_hook: ptr::null(),
        lp_template_name: ptr::null(),
        pv_reserved: ptr::null_mut(),
        dw_reserved: 0,
        flags_ex: 0,
    };
    let ok = unsafe { GetOpenFileNameW(&mut ofn) };
    if ok == 0 {
        return None;
    }
    let parts: Vec<OsString> = buf
        .split(|&c| c == 0)
        .take_while(|s| !s.is_empty())
        .map(OsString::from_wide)
        .collect();
    if parts.is_empty() {
        return None;
    }
    if parts.len() == 1 {
        return Some(vec![PathBuf::from(&parts[0])]);
    }
    let dir = PathBuf::from(&parts[0]);
    Some(parts[1..].iter().map(|name| dir.join(name)).collect())
}

pub fn save_file(title: &str, filter: &str, def_ext: &str) -> Option<PathBuf> {
    let mut buf = vec![0u16; 4096];
    let title_w = to_wide_null(title);
    let filter_w: Vec<u16> = filter.encode_utf16().collect();
    let ext_w = to_wide_null(def_ext);
    let mut ofn = OpenFileNameW {
        l_struct_size: std::mem::size_of::<OpenFileNameW>() as u32,
        hwnd_owner: ptr::null_mut(),
        h_instance: ptr::null_mut(),
        lpstr_filter: filter_w.as_ptr(),
        lpstr_custom_filter: ptr::null_mut(),
        n_max_cust_filter: 0,
        n_filter_index: 1,
        lpstr_file: buf.as_mut_ptr(),
        n_max_file: buf.len() as u32,
        lpstr_file_title: ptr::null_mut(),
        n_max_file_title: 0,
        lpstr_initial_dir: ptr::null(),
        lpstr_title: title_w.as_ptr(),
        flags: OFN_EXPLORER | OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        n_file_offset: 0,
        n_file_extension: 0,
        lpstr_def_ext: ext_w.as_ptr(),
        l_cust_data: 0,
        lpfn_hook: ptr::null(),
        lp_template_name: ptr::null(),
        pv_reserved: ptr::null_mut(),
        dw_reserved: 0,
        flags_ex: 0,
    };
    let ok = unsafe { GetSaveFileNameW(&mut ofn) };
    if ok == 0 {
        return None;
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(PathBuf::from(OsString::from_wide(&buf[..end])))
}

pub fn browse_folder(title: &str) -> Option<PathBuf> {
    let mut display = vec![0u16; 260];
    let mut path = vec![0u16; 4096];
    let title_w = to_wide_null(title);
    let mut bi = BrowseInfoW {
        hwnd_owner: ptr::null_mut(),
        pidl_root: ptr::null(),
        psz_display_name: display.as_mut_ptr(),
        lpsz_title: title_w.as_ptr(),
        ul_flags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE,
        lpfn: ptr::null(),
        l_param: 0,
        i_image: 0,
    };
    let pidl = unsafe { SHBrowseForFolderW(&mut bi) };
    if pidl.is_null() {
        return None;
    }
    let ok = unsafe { SHGetPathFromIDListW(pidl, path.as_mut_ptr()) };
    unsafe { CoTaskMemFree(pidl) };
    if ok == 0 {
        return None;
    }
    let end = path.iter().position(|&c| c == 0).unwrap_or(path.len());
    Some(PathBuf::from(OsString::from_wide(&path[..end])))
}
