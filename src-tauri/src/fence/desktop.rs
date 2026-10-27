//! 桌面条目枚举。
//!
//! 关键设计（移植自 PecoFence）：栅栏**不**去操作 `SysListView32` 里的图标项，
//! 而是直接读桌面文件夹的真实文件，再隐藏原生图标层自绘。这样避免了
//! `LVS_AUTOARRANGE`、PIDL 索引漂移、跨进程内存写入等一堆坑。

use crate::fence::model::{IconKey, Item, ItemKey, Origin};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// 一个磁盘上的桌面条目（尚未与配置里的 [`Item`] 合并）。
#[derive(Clone, Debug)]
pub struct DesktopEntry {
    /// 磁盘上的真实路径。
    pub path: PathBuf,
    /// 显示名（`.lnk`/`.url` 去掉扩展名，与资源管理器一致）。
    pub display_name: String,
    pub origin: Origin,
    pub is_folder: bool,
    pub mtime: i64,
    pub size: u64,
    pub attrs: u32,
    pub is_hidden: bool,
    pub is_system: bool,
}

impl DesktopEntry {
    pub fn key(&self) -> ItemKey {
        ItemKey::from_path(&self.path.to_string_lossy())
    }

    /// 该条目应使用的图标缓存键。
    pub fn icon_key(&self) -> IconKey {
        let ext = self
            .path
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
            .unwrap_or_default();
        // 这些类型的图标与文件内容相关，不能按扩展名共享。
        let content_specific = matches!(
            ext.as_str(),
            ".exe" | ".lnk" | ".url" | ".ico" | ".msi" | ".appref-ms" | ".website"
        ) || self.is_folder;
        if content_specific {
            IconKey::ByContent {
                path: self.path.to_string_lossy().to_string(),
                mtime: self.mtime,
            }
        } else {
            IconKey::ByExt(ext)
        }
    }
}

/// 用户桌面文件夹。
pub fn user_desktop() -> Option<PathBuf> {
    // 优先走 shell 已知文件夹（能正确处理 OneDrive 重定向）。
    #[cfg(target_os = "windows")]
    {
        if let Some(p) = crate::fence::shell_paths::known_folder_desktop() {
            return Some(p);
        }
    }
    dirs::desktop_dir()
}

/// 公共桌面文件夹（`C:\Users\Public\Desktop`）。
pub fn public_desktop() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        if let Some(p) = crate::fence::shell_paths::known_folder_public_desktop() {
            return Some(p);
        }
    }
    None
}

/// 桌面是否可用（文件夹存在）。
pub fn desktop_available() -> bool {
    user_desktop().map(|p| p.is_dir()).unwrap_or(false)
}

/// 枚举用户桌面 + 公共桌面的所有条目。
///
/// 同名条目以用户桌面优先（与资源管理器行为一致）。
pub fn enumerate_desktop() -> Vec<DesktopEntry> {
    let mut out: Vec<DesktopEntry> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();

    if let Some(dir) = user_desktop() {
        for entry in enumerate_folder(&dir, Origin::UserDesktop) {
            let key = entry.display_name.to_lowercase();
            seen.insert(key, out.len());
            out.push(entry);
        }
    }
    if let Some(dir) = public_desktop() {
        for entry in enumerate_folder(&dir, Origin::PublicDesktop) {
            let key = entry.display_name.to_lowercase();
            // 用户桌面已有同名条目则跳过公共桌面的那个。
            if seen.contains_key(&key) {
                continue;
            }
            seen.insert(key, out.len());
            out.push(entry);
        }
    }
    out.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
    });
    out
}

/// 枚举某个文件夹的直接子项（不递归）。
pub fn enumerate_folder(dir: &Path, origin: Origin) -> Vec<DesktopEntry> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in read.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        // 隐藏的系统文件不参与栅栏。
        if name.eq_ignore_ascii_case("desktop.ini") {
            continue;
        }
        if let Some(e) = stat_entry(&path, origin) {
            out.push(e);
        }
    }
    out
}

/// 读取单个条目的元数据。
pub fn stat_entry(path: &Path, origin: Origin) -> Option<DesktopEntry> {
    let meta = std::fs::metadata(path).ok()?;
    let is_folder = meta.is_dir();
    let attrs = file_attrs(path);
    let is_hidden = attrs & 0x2 != 0; // FILE_ATTRIBUTE_HIDDEN
    let is_system = attrs & 0x4 != 0; // FILE_ATTRIBUTE_SYSTEM
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let size = if is_folder { 0 } else { meta.len() };
    let display_name = display_name_for(path);
    Some(DesktopEntry {
        path: path.to_path_buf(),
        display_name,
        origin,
        is_folder,
        mtime,
        size,
        attrs,
        is_hidden,
        is_system,
    })
}

/// 显示名：`.lnk` / `.url` 去掉扩展名，其余保持原名。
pub fn display_name_for(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let lower = name.to_lowercase();
    for ext in [".lnk", ".url", ".website"] {
        if lower.ends_with(ext) && name.len() > ext.len() {
            return name[..name.len() - ext.len()].to_string();
        }
    }
    name
}

/// 读取 Windows 文件属性位。
pub fn file_attrs(path: &Path) -> u32 {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::MetadataExt;
        if let Ok(meta) = std::fs::metadata(path) {
            return meta.file_attributes();
        }
        0
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        0
    }
}

/// 把磁盘条目与已有 [`Item`] 合并：保留 id、打开次数等历史数据。
///
/// 返回 `(合并后的条目表, 新出现的条目 key, 已消失的条目 id)`。
pub fn merge_into(
    existing: &HashMap<Uuid, Item>,
    entries: &[DesktopEntry],
) -> (HashMap<Uuid, Item>, Vec<Uuid>, Vec<Uuid>) {
    let mut by_key: HashMap<ItemKey, Uuid> = HashMap::new();
    for (id, item) in existing {
        by_key.insert(item.key.clone(), *id);
    }

    let mut out: HashMap<Uuid, Item> = HashMap::new();
    let mut created = Vec::new();
    let mut present: Vec<Uuid> = Vec::new();

    for entry in entries {
        let key = entry.key();
        let id = by_key.get(&key).copied().unwrap_or_else(Uuid::new_v4);
        let prev = existing.get(&id);
        let item = Item {
            id,
            key,
            origin: entry.origin,
            display_name: entry.display_name.clone(),
            mtime: entry.mtime,
            is_folder: entry.is_folder,
            attrs: entry.attrs,
            icon_key: entry.icon_key(),
            orphaned_since: None,
            size: entry.size,
            open_count: prev.map(|p| p.open_count).unwrap_or(0),
            last_opened: prev.and_then(|p| p.last_opened),
            path: Some(entry.path.to_string_lossy().to_string()),
        };
        if prev.is_none() {
            created.push(id);
        }
        present.push(id);
        out.insert(id, item);
    }

    // 磁盘上已不存在的条目：标记 orphaned，但保留记录（栅栏成员关系还在）。
    let now = now_secs();
    let mut orphaned = Vec::new();
    for (id, item) in existing {
        if present.contains(id) {
            continue;
        }
        let mut kept = item.clone();
        if kept.orphaned_since.is_none() {
            kept.orphaned_since = Some(now);
            orphaned.push(*id);
        }
        out.insert(*id, kept);
    }

    (out, created, orphaned)
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, is_folder: bool) -> DesktopEntry {
        DesktopEntry {
            path: PathBuf::from(format!("C:\\Users\\A\\Desktop\\{name}")),
            display_name: name.to_string(),
            origin: Origin::UserDesktop,
            is_folder,
            mtime: 1000,
            size: 42,
            attrs: 0,
            is_hidden: false,
            is_system: false,
        }
    }

    #[test]
    fn display_name_strips_shortcut_extension() {
        assert_eq!(display_name_for(Path::new("C:\\d\\Steam.lnk")), "Steam");
        assert_eq!(display_name_for(Path::new("C:\\d\\Site.url")), "Site");
        // 普通文件保留扩展名
        assert_eq!(
            display_name_for(Path::new("C:\\d\\Report.docx")),
            "Report.docx"
        );
        // 只有扩展名的怪文件名不裁
        assert_eq!(display_name_for(Path::new("C:\\d\\.lnk")), ".lnk");
    }

    #[test]
    fn icon_key_is_content_specific_for_shortcuts_and_folders() {
        assert!(matches!(
            entry("Steam.lnk", false).icon_key(),
            IconKey::ByContent { .. }
        ));
        assert!(matches!(
            entry("Photos", true).icon_key(),
            IconKey::ByContent { .. }
        ));
        assert_eq!(
            entry("a.zip", false).icon_key(),
            IconKey::ByExt(".zip".into())
        );
    }

    #[test]
    fn merge_creates_new_items_and_preserves_history() {
        let existing: HashMap<Uuid, Item> = HashMap::new();
        let entries = vec![entry("a.txt", false), entry("b.txt", false)];
        let (items, created, orphaned) = merge_into(&existing, &entries);
        assert_eq!(items.len(), 2);
        assert_eq!(created.len(), 2);
        assert!(orphaned.is_empty());

        // 第二轮：a.txt 保留打开次数，b.txt 消失
        let a_id = *created.first().unwrap();
        let mut with_history = items.clone();
        if let Some(a) = with_history.get_mut(&a_id) {
            a.open_count = 7;
        }
        let (items2, created2, orphaned2) = merge_into(&with_history, &[entry("a.txt", false)]);
        assert_eq!(items2.len(), 2, "消失的条目应保留记录");
        assert!(created2.is_empty());
        assert_eq!(orphaned2.len(), 1);
        let a2 = items2.values().find(|i| i.open_count == 7).unwrap();
        assert_eq!(a2.open_count, 7);
        let gone = items2
            .values()
            .find(|i| i.orphaned_since.is_some())
            .unwrap();
        assert_eq!(gone.display_name, "b.txt");
    }

    #[test]
    fn merge_revives_orphaned_item_without_duplicating() {
        let entries = vec![entry("a.txt", false)];
        let (items, created, _) = merge_into(&HashMap::new(), &entries);
        let mut orphaned_items = items.clone();
        for v in orphaned_items.values_mut() {
            v.orphaned_since = Some(123);
        }
        let (items2, created2, _) = merge_into(&orphaned_items, &entries);
        assert_eq!(items2.len(), 1);
        assert!(created2.is_empty());
        assert!(items2.values().next().unwrap().orphaned_since.is_none());
        assert_eq!(created.len(), 1);
    }

    #[test]
    fn merge_is_case_insensitive_on_path() {
        let entries = vec![entry("Report.TXT", false)];
        let (items, created, _) = merge_into(&HashMap::new(), &entries);
        assert_eq!(created.len(), 1);
        // 同名不同大小写不应新建
        let mut lower = entry("Report.TXT", false);
        lower.path = PathBuf::from("C:\\Users\\A\\Desktop\\report.txt");
        let (items2, created2, _) = merge_into(&items, &[lower]);
        assert_eq!(items2.len(), 1);
        assert!(created2.is_empty());
    }
}
