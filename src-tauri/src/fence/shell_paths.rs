//! Shell 已知文件夹路径查询。
//!
//! 桌面可能被 OneDrive 重定向到别处，`dirs::desktop_dir()` 在部分环境下会取错，
//! 因此优先走 `SHGetKnownFolderPath`。

use std::path::PathBuf;

#[cfg(target_os = "windows")]
mod win {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::path::PathBuf;
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::{
        FOLDERID_Desktop, FOLDERID_PublicDesktop, SHGetKnownFolderPath, KF_FLAG_DEFAULT,
    };

    fn known_folder(id: &windows_sys::core::GUID) -> Option<PathBuf> {
        unsafe {
            let mut raw: *mut u16 = std::ptr::null_mut();
            let hr =
                SHGetKnownFolderPath(id, KF_FLAG_DEFAULT as u32, std::ptr::null_mut(), &mut raw);
            if hr != 0 || raw.is_null() {
                return None;
            }
            let len = (0..).take_while(|&i| *raw.add(i) != 0).count();
            let slice = std::slice::from_raw_parts(raw, len);
            let s = OsString::from_wide(slice);
            CoTaskMemFree(raw as *mut core::ffi::c_void);
            let p = PathBuf::from(s);
            if p.as_os_str().is_empty() {
                None
            } else {
                Some(p)
            }
        }
    }

    pub fn desktop() -> Option<PathBuf> {
        known_folder(&FOLDERID_Desktop)
    }

    pub fn public_desktop() -> Option<PathBuf> {
        known_folder(&FOLDERID_PublicDesktop)
    }
}

/// 用户桌面文件夹（shell 已知文件夹）。
pub fn known_folder_desktop() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        win::desktop()
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

/// 公共桌面文件夹。
pub fn known_folder_public_desktop() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        win::public_desktop()
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_path_is_absolute_when_found() {
        if let Some(p) = known_folder_desktop() {
            assert!(p.is_absolute(), "桌面路径应为绝对路径: {p:?}");
        }
    }
}
