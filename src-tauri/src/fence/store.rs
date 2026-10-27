//! 栅栏配置持久化。
//!
//! 写入策略（与便签模块一致）：先写 `.tmp` → `fsync` → `rename` 覆盖，
//! 保证崩溃时不会留下半截 JSON。读取失败时备份损坏文件并回落到默认配置。

use crate::fence::model::Config;
use std::io::Write;
use std::path::{Path, PathBuf};

/// 配置文件名。
pub const CONFIG_FILE: &str = "fences.json";
/// 损坏文件备份后缀。
const CORRUPT_SUFFIX: &str = ".corrupt";

/// 栅栏配置目录：`%APPDATA%\Renown\fences`。
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::env::temp_dir())
        .join("Renown")
        .join("fences")
}

/// 配置文件完整路径。
pub fn config_path() -> PathBuf {
    config_dir().join(CONFIG_FILE)
}

/// 加载配置；文件不存在或损坏时返回默认配置。
///
/// 返回 `(config, 是否发生过恢复)`，便于上层提示用户。
pub fn load() -> (Config, bool) {
    load_from(&config_path())
}

/// 从指定路径加载（便于测试）。
pub fn load_from(path: &Path) -> (Config, bool) {
    let Ok(bytes) = std::fs::read(path) else {
        return (Config::default(), false);
    };
    if bytes.is_empty() {
        return (Config::default(), false);
    }
    match serde_json::from_slice::<Config>(&bytes) {
        Ok(cfg) => (cfg, false),
        Err(err) => {
            // 备份损坏文件，避免用户数据被静默丢弃。
            let backup = path.with_extension(format!("json{CORRUPT_SUFFIX}"));
            let _ = std::fs::copy(path, &backup);
            eprintln!("[fence] 配置解析失败，已备份到 {}: {err}", backup.display());
            (Config::default(), true)
        }
    }
}

/// 原子保存配置。
pub fn save(config: &Config) -> std::io::Result<()> {
    save_to(&config_path(), config)
}

/// 原子写入指定路径（便于测试）。
pub fn save_to(path: &Path, config: &Config) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_vec_pretty(config)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let tmp = path.with_extension("json.tmp");
    {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(&json)?;
        // 落盘，避免 rename 后内容仍在页缓存里。
        file.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fence::model::{new_inbox, Layout};

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("renown-fence-store-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn load_missing_file_returns_default() {
        let dir = temp_dir("missing");
        let (cfg, recovered) = load_from(&dir.join("nope.json"));
        assert!(!recovered);
        assert!(cfg.layouts.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = temp_dir("roundtrip");
        let path = dir.join("fences.json");
        let mut cfg = Config::default();
        let mut layout = Layout::new(vec![]);
        layout.fences.push(new_inbox("D1", 1920.0, 1080.0));
        cfg.layouts.push(layout);

        save_to(&path, &cfg).unwrap();
        let (back, recovered) = load_from(&path);
        assert!(!recovered);
        assert_eq!(back.layouts.len(), 1);
        assert_eq!(back.layouts[0].fences[0].title, "桌面");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn save_leaves_no_tmp_file() {
        let dir = temp_dir("notmp");
        let path = dir.join("fences.json");
        save_to(&path, &Config::default()).unwrap();
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "不应残留 .tmp 文件");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn corrupt_file_is_backed_up_and_recovered() {
        let dir = temp_dir("corrupt");
        let path = dir.join("fences.json");
        std::fs::write(&path, b"{ this is not json").unwrap();
        let (cfg, recovered) = load_from(&path);
        assert!(recovered, "应报告已恢复");
        assert!(cfg.layouts.is_empty());
        let backup = path.with_extension("json.corrupt");
        assert!(backup.exists(), "损坏文件应被备份");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn empty_file_is_treated_as_default_without_backup() {
        let dir = temp_dir("empty");
        let path = dir.join("fences.json");
        std::fs::write(&path, b"").unwrap();
        let (cfg, recovered) = load_from(&path);
        assert!(!recovered);
        assert!(cfg.layouts.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn save_overwrites_existing_atomically() {
        let dir = temp_dir("overwrite");
        let path = dir.join("fences.json");
        let mut cfg = Config::default();
        save_to(&path, &cfg).unwrap();
        cfg.settings.icon_size = 64;
        save_to(&path, &cfg).unwrap();
        let (back, _) = load_from(&path);
        assert_eq!(back.settings.icon_size, 64);
        let _ = std::fs::remove_dir_all(dir);
    }
}
