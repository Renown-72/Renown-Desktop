//! 桌面图标提取与缓存。
//!
//! 用 `IShellItemImageFactory::GetImage` 取图标/缩略图 —— 与资源管理器同源，
//! 因此自定义图标、快捷方式目标图标、图片缩略图的表现都和真实桌面一致。
//! 结果编码为 PNG 并落盘缓存，前端按 `IconKey` 请求。

use crate::fence::model::IconKey;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// 提取出的图标位图（RGBA8，未预乘）。
#[derive(Clone, Debug)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// 图标缓存目录：`%LOCALAPPDATA%\Renown\icon-cache`。
pub fn cache_dir() -> PathBuf {
    let base = dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(std::env::temp_dir);
    base.join("Renown").join("icon-cache")
}

/// 把 [`IconKey`] 映射为稳定的缓存文件名（不含扩展名）。
pub fn cache_key(key: &IconKey) -> String {
    let raw = match key {
        IconKey::ByExt(ext) => format!("ext:{ext}"),
        IconKey::ByContent { path, mtime } => format!("file:{path}:{mtime}"),
    };
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let digest = hasher.finalize();
    // 取前 16 字节十六进制，足够避免碰撞且文件名短。
    digest[..16].iter().map(|b| format!("{b:02x}")).collect()
}

/// 读取缓存中的 PNG；未命中返回 `None`。
pub fn read_cached(key: &IconKey) -> Option<Vec<u8>> {
    let path = cache_dir().join(format!("{}.png", cache_key(key)));
    std::fs::read(path).ok().filter(|b| !b.is_empty())
}

/// 写入缓存（原子写：先写 .tmp 再 rename）。
pub fn write_cached(key: &IconKey, png: &[u8]) -> std::io::Result<()> {
    let dir = cache_dir();
    std::fs::create_dir_all(&dir)?;
    let final_path = dir.join(format!("{}.png", cache_key(key)));
    let tmp = dir.join(format!(".{}.tmp", cache_key(key)));
    std::fs::write(&tmp, png)?;
    std::fs::rename(&tmp, &final_path)?;
    Ok(())
}

/// 清空图标缓存目录。
pub fn clear_cache() -> std::io::Result<usize> {
    let dir = cache_dir();
    if !dir.is_dir() {
        return Ok(0);
    }
    let mut n = 0;
    for entry in std::fs::read_dir(&dir)?.flatten() {
        if std::fs::remove_file(entry.path()).is_ok() {
            n += 1;
        }
    }
    Ok(n)
}

/// 取图标 PNG 字节：先查缓存，未命中则从 shell 提取并写缓存。
///
/// `size_px` 是期望的像素尺寸（会按 DPI 放大后传入）。
pub fn png_for(path: &Path, key: &IconKey, size_px: u32) -> Option<Vec<u8>> {
    if let Some(hit) = read_cached(key) {
        return Some(hit);
    }
    let img = extract(path, size_px)?;
    let png = encode_png(&img)?;
    let _ = write_cached(key, &png);
    Some(png)
}

/// 用 shell 提取图标位图。
#[cfg(target_os = "windows")]
pub fn extract(path: &Path, size_px: u32) -> Option<RgbaImage> {
    win::extract(path, size_px)
}

#[cfg(not(target_os = "windows"))]
pub fn extract(_path: &Path, _size_px: u32) -> Option<RgbaImage> {
    None
}

/// 编码为 PNG。
pub fn encode_png(img: &RgbaImage) -> Option<Vec<u8>> {
    use image::ImageEncoder;
    let mut out = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut out);
    encoder
        .write_image(
            &img.rgba,
            img.width,
            img.height,
            image::ExtendedColorType::Rgba8,
        )
        .ok()?;
    Some(out)
}

#[cfg(target_os = "windows")]
mod win {
    use super::RgbaImage;
    use std::path::Path;
    use windows::core::HSTRING;
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{
        DeleteObject, GetDC, GetDIBits, ReleaseDC, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS, HBITMAP, HGDIOBJ,
    };
    use windows::Win32::System::Com::{
        CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
    };
    use windows::Win32::UI::Shell::{
        IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF, SIIGBF_BIGGERSIZEOK,
        SIIGBF_ICONONLY,
    };

    /// COM 作用域守卫：进入时初始化 STA，离开时反初始化（仅当本线程首次初始化）。
    struct ComScope {
        should_uninit: bool,
    }

    impl ComScope {
        fn enter() -> Self {
            // RPC_E_CHANGED_MODE (0x8001_0106) 表示该线程已是别的套间模型，可继续用。
            let hr =
                unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
            let should_uninit = hr.is_ok();
            Self { should_uninit }
        }
    }

    impl Drop for ComScope {
        fn drop(&mut self) {
            if self.should_uninit {
                unsafe { CoUninitialize() };
            }
        }
    }

    pub fn extract(path: &Path, size_px: u32) -> Option<RgbaImage> {
        let size = size_px.clamp(16, 256);
        let _com = ComScope::enter();
        let factory: IShellItemImageFactory =
            unsafe { SHCreateItemFromParsingName(&HSTRING::from(path.as_os_str()), None).ok()? };
        let flags = SIIGBF(SIIGBF_BIGGERSIZEOK.0 | SIIGBF_ICONONLY.0);
        let hbitmap: HBITMAP = unsafe {
            factory
                .GetImage(
                    SIZE {
                        cx: size as i32,
                        cy: size as i32,
                    },
                    flags,
                )
                .ok()?
        };
        let result = unsafe { hbitmap_to_rgba(hbitmap, size) };
        unsafe {
            let _ = DeleteObject(HGDIOBJ(hbitmap.0));
        }
        result
    }

    /// HBITMAP → 未预乘 RGBA。
    unsafe fn hbitmap_to_rgba(hbitmap: HBITMAP, size_px: u32) -> Option<RgbaImage> {
        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = size_px as i32;
        // 负高度 = 自上而下，省去翻转。
        bmi.bmiHeader.biHeight = -(size_px as i32);
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB.0;

        let mut buf = vec![0u8; (size_px * size_px * 4) as usize];
        let hdc = unsafe { GetDC(None) };
        if hdc.is_invalid() {
            return None;
        }
        let got = unsafe {
            GetDIBits(
                hdc,
                hbitmap,
                0,
                size_px,
                Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
                &mut bmi,
                DIB_RGB_COLORS,
            )
        };
        unsafe { ReleaseDC(None, hdc) };
        if got == 0 {
            return None;
        }

        // shell 返回预乘 BGRA：转成未预乘 RGBA。
        let mut out = Vec::with_capacity(buf.len());
        for px in buf.chunks_exact(4) {
            let (b, g, r, a) = (px[0], px[1], px[2], px[3]);
            if a == 0 {
                out.extend_from_slice(&[0, 0, 0, 0]);
            } else if a == 255 {
                out.extend_from_slice(&[r, g, b, 255]);
            } else {
                let inv = 255.0 / a as f32;
                out.extend_from_slice(&[
                    (r as f32 * inv).min(255.0) as u8,
                    (g as f32 * inv).min(255.0) as u8,
                    (b as f32 * inv).min(255.0) as u8,
                    a,
                ]);
            }
        }
        Some(RgbaImage {
            width: size_px,
            height: size_px,
            rgba: out,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_is_stable_and_distinct() {
        let a = cache_key(&IconKey::ByExt(".zip".into()));
        let b = cache_key(&IconKey::ByExt(".zip".into()));
        let c = cache_key(&IconKey::ByExt(".rar".into()));
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 32);
    }

    #[test]
    fn cache_key_differs_by_mtime_for_content_icons() {
        let a = cache_key(&IconKey::ByContent {
            path: "C:\\a.exe".into(),
            mtime: 1,
        });
        let b = cache_key(&IconKey::ByContent {
            path: "C:\\a.exe".into(),
            mtime: 2,
        });
        assert_ne!(a, b);
    }

    #[test]
    fn encode_png_produces_png_signature() {
        let img = RgbaImage {
            width: 2,
            height: 2,
            rgba: vec![255; 16],
        };
        let png = encode_png(&img).expect("encode");
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    }

    #[test]
    fn cache_roundtrip() {
        let key = IconKey::ByExt(".renown-test-ext".into());
        let img = RgbaImage {
            width: 1,
            height: 1,
            rgba: vec![10, 20, 30, 255],
        };
        let png = encode_png(&img).unwrap();
        write_cached(&key, &png).expect("write cache");
        let back = read_cached(&key).expect("read cache");
        assert_eq!(back, png);
        // 清理，避免污染真实缓存目录
        let path = cache_dir().join(format!("{}.png", cache_key(&key)));
        let _ = std::fs::remove_file(path);
    }
}
