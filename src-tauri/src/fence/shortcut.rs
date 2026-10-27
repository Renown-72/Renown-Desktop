//! `.lnk` / `.url` 快捷方式目标解析。
//!
//! 规则引擎的「快捷方式目标」条件需要知道 `.lnk` 指向哪个可执行文件，
//! 才能把「指向图片查看器的快捷方式」和「图片文件」区分开。
//!
//! `.lnk` 采用**直接解析二进制格式**（MS-SHLLINK）而不是 `IShellLink` COM：
//! 只读 `LinkInfo` 里的本地基础路径，够用且无 COM 套间要求。

use std::path::Path;

/// 解析结果。
#[derive(Clone, Debug, PartialEq)]
pub struct ShortcutInfo {
    /// 目标路径（本地文件或 URL）。
    pub target: String,
    /// 目标扩展名（小写含点）。
    pub target_ext: Option<String>,
}

/// 解析 `.lnk` / `.url` 的目标。
pub fn resolve(path: &Path) -> Option<ShortcutInfo> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "lnk" => resolve_lnk(path),
        "url" => resolve_url(path),
        _ => None,
    }
}

/// 从路径推出小写含点扩展名。
fn ext_with_dot(target: &str) -> Option<String> {
    let name = target.rsplit(['\\', '/']).next().unwrap_or(target);
    let (_, ext) = name.rsplit_once('.')?;
    if ext.is_empty() || ext.len() > 12 {
        return None;
    }
    Some(format!(".{}", ext.to_lowercase()))
}

fn make_info(target: String) -> ShortcutInfo {
    ShortcutInfo {
        target_ext: ext_with_dot(&target),
        target,
    }
}

// ---------------------------------------------------------------------------
// .url（纯文本 INI）
// ---------------------------------------------------------------------------

fn resolve_url(path: &Path) -> Option<ShortcutInfo> {
    let text = read_text(path)?;
    for line in text.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("URL=") {
            let v = v.trim();
            if !v.is_empty() {
                return Some(ShortcutInfo {
                    target: v.to_string(),
                    target_ext: None,
                });
            }
        }
    }
    None
}

/// 读取文本文件，兼容 UTF-16 BOM 与 UTF-8。
fn read_text(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return Some(String::from_utf16_lossy(&units));
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        return Some(String::from_utf16_lossy(&units));
    }
    Some(match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => String::from_utf8_lossy(e.as_bytes()).into_owned(),
    })
}

// ---------------------------------------------------------------------------
// .lnk（MS-SHLLINK 二进制）
// ---------------------------------------------------------------------------

/// `LinkFlags` 位。
const HAS_LINK_TARGET_ID_LIST: u32 = 0x0000_0001;
const HAS_LINK_INFO: u32 = 0x0000_0002;
const HAS_RELATIVE_PATH: u32 = 0x0000_0008;

/// `LinkInfoFlags` 位。
const VOLUME_ID_AND_LOCAL_BASE_PATH: u32 = 0x0000_0001;

fn resolve_lnk(path: &Path) -> Option<ShortcutInfo> {
    let data = std::fs::read(path).ok()?;
    parse_lnk(&data).map(make_info)
}

/// 解析 `.lnk` 字节流，返回目标路径。
pub fn parse_lnk(data: &[u8]) -> Option<String> {
    // ShellLinkHeader 固定 76 字节
    if data.len() < 76 {
        return None;
    }
    let header_size = read_u32(data, 0)?;
    if header_size != 0x4C {
        return None;
    }
    // LinkCLSID 必须是 00021401-0000-0000-C000-000000000046
    const CLSID: [u8; 16] = [
        0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x46,
    ];
    if data.get(4..20)? != CLSID {
        return None;
    }
    let flags = read_u32(data, 20)?;
    let mut pos = 76usize;

    // 跳过 LinkTargetIDList
    if flags & HAS_LINK_TARGET_ID_LIST != 0 {
        let size = read_u16(data, pos)? as usize;
        pos = pos.checked_add(2)?.checked_add(size)?;
    }

    // LinkInfo 优先（本地绝对路径最可靠）
    if flags & HAS_LINK_INFO != 0 {
        if let Some(target) = parse_link_info(data, pos) {
            return Some(target);
        }
        // 跳过 LinkInfo 继续找相对路径
        if let Some(size) = read_u32(data, pos) {
            pos = pos.checked_add(size as usize)?;
        }
    }

    // 回退到 RelativePath 字符串
    if flags & HAS_RELATIVE_PATH != 0 {
        // 先跳过可能存在的 Name 字符串
        if flags & 0x0000_0004 != 0 {
            let n = read_u16(data, pos)? as usize;
            pos = pos.checked_add(2)?.checked_add(n * 2)?;
        }
        let n = read_u16(data, pos)? as usize;
        let start = pos.checked_add(2)?;
        let end = start.checked_add(n * 2)?;
        let slice = data.get(start..end)?;
        let units: Vec<u16> = slice
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let rel = String::from_utf16_lossy(&units);
        if !rel.is_empty() {
            return Some(rel);
        }
    }
    None
}

/// 解析 LinkInfo 结构，返回「本地基础路径 + 公共后缀」。
fn parse_link_info(data: &[u8], start: usize) -> Option<String> {
    let link_info_size = read_u32(data, start)? as usize;
    if link_info_size == 0 || start.checked_add(link_info_size)? > data.len() {
        return None;
    }
    let header_size = read_u32(data, start + 4)? as usize;
    let info_flags = read_u32(data, start + 8)?;
    if info_flags & VOLUME_ID_AND_LOCAL_BASE_PATH == 0 {
        return None;
    }
    let local_base_offset = read_u32(data, start + 16)? as usize;
    let common_suffix_offset = read_u32(data, start + 24)? as usize;

    // 头足够长时优先用 Unicode 版本
    let (base, suffix) = if header_size >= 0x24 {
        let base_u = read_u32(data, start + 28).map(|v| v as usize).unwrap_or(0);
        let suffix_u = read_u32(data, start + 32).map(|v| v as usize).unwrap_or(0);
        let base = if base_u != 0 {
            read_utf16_z(data, start + base_u)
        } else {
            read_ansi_z(data, start + local_base_offset)
        };
        let suffix = if suffix_u != 0 {
            read_utf16_z(data, start + suffix_u)
        } else {
            read_ansi_z(data, start + common_suffix_offset)
        };
        (base, suffix)
    } else {
        (
            read_ansi_z(data, start + local_base_offset),
            read_ansi_z(data, start + common_suffix_offset),
        )
    };

    let base = base?;
    if base.is_empty() {
        return None;
    }
    let mut target = base;
    if let Some(suffix) = suffix {
        if !suffix.is_empty() {
            target.push_str(&suffix);
        }
    }
    Some(target)
}

fn read_u16(data: &[u8], at: usize) -> Option<u16> {
    let b = data.get(at..at + 2)?;
    Some(u16::from_le_bytes([b[0], b[1]]))
}

fn read_u32(data: &[u8], at: usize) -> Option<u32> {
    let b = data.get(at..at + 4)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// 读以 NUL 结尾的 UTF-16LE 字符串。
fn read_utf16_z(data: &[u8], at: usize) -> Option<String> {
    if at >= data.len() {
        return None;
    }
    let mut units = Vec::new();
    let mut i = at;
    while i + 1 < data.len() {
        let u = u16::from_le_bytes([data[i], data[i + 1]]);
        if u == 0 {
            break;
        }
        units.push(u);
        i += 2;
    }
    Some(String::from_utf16_lossy(&units))
}

/// 读以 NUL 结尾的 ANSI 字符串（按系统代码页解码）。
fn read_ansi_z(data: &[u8], at: usize) -> Option<String> {
    if at >= data.len() {
        return None;
    }
    let end = data[at..]
        .iter()
        .position(|&b| b == 0)
        .map(|i| at + i)
        .unwrap_or(data.len());
    Some(ansi_to_string(&data[at..end]))
}

/// 按系统 ANSI 代码页解码字节。
fn ansi_to_string(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Globalization::{MultiByteToWideChar, CP_ACP};
        unsafe {
            let n = MultiByteToWideChar(
                CP_ACP,
                0,
                bytes.as_ptr(),
                bytes.len() as i32,
                std::ptr::null_mut(),
                0,
            );
            if n <= 0 {
                return String::from_utf8_lossy(bytes).into_owned();
            }
            let mut w = vec![0u16; n as usize];
            let n2 = MultiByteToWideChar(
                CP_ACP,
                0,
                bytes.as_ptr(),
                bytes.len() as i32,
                w.as_mut_ptr(),
                n,
            );
            String::from_utf16_lossy(&w[..n2.max(0) as usize])
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_file(name: &str, content: &[u8]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("renown-shortcut-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn url_shortcut_resolves_target() {
        let p = temp_file(
            "test.url",
            b"[InternetShortcut]\r\nURL=https://example.com/x\r\n",
        );
        let info = resolve(&p).expect("should resolve");
        assert_eq!(info.target, "https://example.com/x");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn url_shortcut_handles_utf16_bom() {
        let mut bytes = vec![0xFF, 0xFE];
        for u in "[InternetShortcut]\nURL=https://例子.com\n".encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        let p = temp_file("utf16.url", &bytes);
        let info = resolve(&p).expect("should resolve");
        assert_eq!(info.target, "https://例子.com");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn non_shortcut_returns_none() {
        let p = temp_file("plain.txt", b"hello");
        assert!(resolve(&p).is_none());
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn ext_with_dot_normalizes() {
        assert_eq!(
            ext_with_dot(r"C:\Program Files\App\App.EXE").as_deref(),
            Some(".exe")
        );
        assert_eq!(ext_with_dot("noext"), None);
    }

    #[test]
    fn rejects_truncated_lnk() {
        assert!(parse_lnk(&[]).is_none());
        assert!(parse_lnk(&[0u8; 40]).is_none());
    }

    #[test]
    fn rejects_lnk_with_wrong_clsid() {
        let mut data = vec![0u8; 100];
        data[0..4].copy_from_slice(&0x4Cu32.to_le_bytes());
        // CLSID 全零，不匹配
        assert!(parse_lnk(&data).is_none());
    }

    /// 构造一个最小可用的 .lnk：只有 LinkInfo + 本地基础路径（ANSI）。
    fn build_lnk_with_local_base(base: &str) -> Vec<u8> {
        const CLSID: [u8; 16] = [
            0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x46,
        ];
        let mut out = vec![0u8; 76];
        out[0..4].copy_from_slice(&0x4Cu32.to_le_bytes());
        out[4..20].copy_from_slice(&CLSID);
        // HasLinkInfo
        out[20..24].copy_from_slice(&HAS_LINK_INFO.to_le_bytes());

        // LinkInfo：头 0x1C 字节（无 Unicode 字段）
        let header_size: u32 = 0x1C;
        let base_bytes = base.as_bytes();
        let base_offset = header_size;
        let suffix_offset = base_offset + base_bytes.len() as u32 + 1;
        let total = suffix_offset + 1; // 后缀为空，仅一个 NUL

        let mut li = Vec::new();
        li.extend_from_slice(&total.to_le_bytes()); // LinkInfoSize
        li.extend_from_slice(&header_size.to_le_bytes()); // LinkInfoHeaderSize
        li.extend_from_slice(&VOLUME_ID_AND_LOCAL_BASE_PATH.to_le_bytes()); // Flags
        li.extend_from_slice(&0u32.to_le_bytes()); // VolumeIDOffset
        li.extend_from_slice(&base_offset.to_le_bytes()); // LocalBasePathOffset
        li.extend_from_slice(&0u32.to_le_bytes()); // CommonNetworkRelativeLinkOffset
        li.extend_from_slice(&suffix_offset.to_le_bytes()); // CommonPathSuffixOffset
        li.extend_from_slice(base_bytes);
        li.push(0); // base 的 NUL
        li.push(0); // suffix 的 NUL

        out.extend_from_slice(&li);
        out
    }

    #[test]
    fn parses_lnk_local_base_path() {
        let data = build_lnk_with_local_base(r"C:\Program Files\App\app.exe");
        let target = parse_lnk(&data).expect("should parse");
        assert_eq!(target, r"C:\Program Files\App\app.exe");
        let info = make_info(target);
        assert_eq!(info.target_ext.as_deref(), Some(".exe"));
    }

    #[test]
    fn ansi_decoder_handles_ascii() {
        assert_eq!(ansi_to_string(b"C:\\a\\b.exe"), r"C:\a\b.exe");
        assert_eq!(ansi_to_string(b""), "");
    }
}
