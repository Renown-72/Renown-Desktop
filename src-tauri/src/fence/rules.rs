//! 自动归类规则引擎。
//!
//! 移植自 PecoFence（Apache-2.0）的 `crates/core/src/rules.rs`，纯函数式：
//! 输入一个条目的 [`ItemFacts`]，输出 [`Decision`]。不触碰文件系统与 UI。

use crate::fence::model::{FenceId, Origin, RuleId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Target {
    Inbox,
    Fence(FenceId),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleSet {
    #[serde(default = "default_target")]
    pub default_target: Target,
    /// 桌面变化时是否持续重跑规则。
    #[serde(default = "default_true")]
    pub keep_updated: bool,
    /// 应用规则时，已在某个栅栏里但无规则匹配的条目怎么处理。
    #[serde(default)]
    pub unmatched: Unmatched,
    /// 有序；首个匹配的规则胜出（Target 类优先）。
    #[serde(default)]
    pub list: Vec<Rule>,
}

fn default_target() -> Target {
    Target::Inbox
}
fn default_true() -> bool {
    true
}

impl Default for RuleSet {
    fn default() -> Self {
        Self {
            default_target: Target::Inbox,
            keep_updated: true,
            unmatched: Unmatched::DefaultTarget,
            list: Vec::new(),
        }
    }
}

/// 无规则匹配时的处理方式。新条目（尚不在任何栅栏）总是去默认目标。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Unmatched {
    /// 回到默认目标（「桌面」收件箱）。
    #[default]
    DefaultTarget,
    /// 留在当前栅栏。
    Stay,
}

/// 规则优先级类别。`Target` 类总是先于其他类求值。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Class {
    Target,
    Type,
    Name,
    Time,
    Custom,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: RuleId,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub target: Target,
    /// 所有条件必须同时成立（AND）。
    #[serde(default)]
    pub all_of: Vec<Cond>,
    #[serde(default = "default_class")]
    pub priority_class: Class,
    /// 该规则来自哪个「快速添加」模板。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
}

fn default_class() -> Class {
    Class::Custom
}

impl Rule {
    pub fn new(name: &str, target: Target, all_of: Vec<Cond>) -> Self {
        let priority_class = if all_of
            .iter()
            .any(|c| matches!(c, Cond::ShortcutTarget { .. }))
        {
            Class::Target
        } else if all_of
            .iter()
            .any(|c| matches!(c, Cond::Type(_) | Cond::Ext(_)))
        {
            Class::Type
        } else if all_of
            .iter()
            .any(|c| matches!(c, Cond::Name { .. } | Cond::Glob(_) | Cond::ExactName(_)))
        {
            Class::Name
        } else if all_of.iter().any(|c| {
            matches!(
                c,
                Cond::CreatedTime { .. } | Cond::CreatedWeekday(_) | Cond::IdleDays { .. }
            )
        }) {
            Class::Time
        } else {
            Class::Custom
        };
        Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            enabled: true,
            target,
            all_of,
            priority_class,
            template: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TypeCategory {
    Programs,
    Folders,
    Documents,
    Images,
    Music,
    Video,
    Archives,
    Shortcuts,
    /// 安装包：`.msi`/`.msix`/`.appx` 家族，以及名字含 setup/install 的 `.exe`。
    Installers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StrOp {
    Contains,
    NotContains,
    StartsWith,
    EndsWith,
    Is,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "cond", content = "value")]
pub enum Cond {
    Type(Vec<TypeCategory>),
    Ext(Vec<String>),
    ExactName(Vec<String>),
    Name {
        op: StrOp,
        value: String,
    },
    Glob(String),
    ShortcutTarget {
        op: StrOp,
        value: String,
    },
    FoldersOnly,
    FilesOnly,
    /// 既不是程序、也不是快捷方式或命名空间条目的普通文件。
    PlainFiles,
    SizeMb {
        min: Option<f64>,
        max: Option<f64>,
    },
    /// 本地时间的「距零点分钟数」；`from > to` 时跨零点。
    CreatedTime {
        from_min: u16,
        to_min: u16,
    },
    /// 0 = 周一 … 6 = 周日。
    CreatedWeekday(Vec<u8>),
    Origin(Origin),
    /// 距「最后修改或最后从栅栏打开」的整天数 ≥ `min`。年龄未知时永不匹配。
    IdleDays {
        min: u32,
    },
}

/// 规则引擎可查看的条目信息。
#[derive(Clone, Debug, Default)]
pub struct ItemFacts {
    /// 含扩展名的文件名（`Report.docx`、`Steam.lnk`）。
    pub file_name: String,
    pub is_folder: bool,
    pub size_bytes: u64,
    /// 快捷方式解析后的目标路径 / URL。
    pub shortcut_target: Option<String>,
    /// 快捷方式目标的扩展名（小写含点，如 `.exe`）。
    pub shortcut_target_ext: Option<String>,
    /// 创建时间：本地距零点分钟数。
    pub created_minutes_local: Option<u16>,
    /// 创建时间：星期（0 = 周一）。
    pub created_weekday: Option<u8>,
    pub origin: Origin,
    pub is_hidden: bool,
    pub is_system: bool,
    /// 距「最后写入与最后打开中较晚者」的整天数。
    pub idle_days: Option<u32>,
    /// 距「创建与最后写入中较晚者」的秒数。
    pub age_secs: Option<u64>,
}

/// 永不路由的扩展名（下载中的文件）。
pub const TEMPORARY_EXTS: &[&str] = &[
    ".crdownload",
    ".part",
    ".partial",
    ".download",
    ".!ut",
    ".opdownload",
];

pub fn ext_of(file_name: &str) -> String {
    match file_name.rfind('.') {
        Some(i) if i > 0 => file_name[i..].to_lowercase(),
        _ => String::new(),
    }
}

/// `.tmp` 文件需存在多久才允许被规则路由：Office 的「安全保存」会写一个 .tmp
/// 再立刻改名覆盖文档。
pub const TMP_SETTLE_SECS: u64 = 60;

/// 是否为临时/在途文件（规则应跳过）。
pub fn is_temporary(file_name: &str, age_secs: Option<u64>) -> bool {
    let lower = file_name.to_lowercase();
    lower.starts_with("~$")
        || TEMPORARY_EXTS.iter().any(|e| lower.ends_with(e))
        || (lower.ends_with(".tmp") && !age_secs.is_some_and(|s| s >= TMP_SETTLE_SECS))
}

/// [`classify`] 报告类别的顺序。
pub const CLASSIFY_ORDER: &[TypeCategory] = &[
    TypeCategory::Folders,
    TypeCategory::Programs,
    TypeCategory::Installers,
    TypeCategory::Shortcuts,
    TypeCategory::Documents,
    TypeCategory::Images,
    TypeCategory::Music,
    TypeCategory::Video,
    TypeCategory::Archives,
];

/// 条目落入 [`CLASSIFY_ORDER`] 的第一个类别。
pub fn classify(facts: &ItemFacts) -> Option<TypeCategory> {
    CLASSIFY_ORDER
        .iter()
        .copied()
        .find(|cat| category_matches(*cat, facts))
}

/// 类别的 camelCase 线名（`programs`、`folders` …）。
pub fn category_name(cat: TypeCategory) -> &'static str {
    match cat {
        TypeCategory::Programs => "programs",
        TypeCategory::Folders => "folders",
        TypeCategory::Documents => "documents",
        TypeCategory::Images => "images",
        TypeCategory::Music => "music",
        TypeCategory::Video => "video",
        TypeCategory::Archives => "archives",
        TypeCategory::Shortcuts => "shortcuts",
        TypeCategory::Installers => "installers",
    }
}

pub fn category_matches(cat: TypeCategory, facts: &ItemFacts) -> bool {
    let ext = ext_of(&facts.file_name);
    let target_ext = facts.shortcut_target_ext.as_deref().unwrap_or("");
    match cat {
        TypeCategory::Folders => facts.is_folder,
        TypeCategory::Programs => {
            !facts.is_folder
                && (matches!(
                    ext.as_str(),
                    ".exe" | ".msi" | ".bat" | ".cmd" | ".appref-ms"
                ) || (ext == ".lnk"
                    && matches!(target_ext, ".exe" | ".msi" | ".bat" | ".cmd" | "")))
        }
        TypeCategory::Shortcuts => {
            !facts.is_folder && matches!(ext.as_str(), ".lnk" | ".url" | ".website")
        }
        TypeCategory::Documents => {
            !facts.is_folder
                && matches!(
                    ext.as_str(),
                    ".doc"
                        | ".docx"
                        | ".xls"
                        | ".xlsx"
                        | ".ppt"
                        | ".pptx"
                        | ".pdf"
                        | ".txt"
                        | ".md"
                        | ".rtf"
                        | ".odt"
                        | ".ods"
                        | ".odp"
                        | ".csv"
                        | ".tsv"
                        | ".one"
                        | ".epub"
                        | ".json"
                        | ".xml"
                )
        }
        TypeCategory::Images => {
            !facts.is_folder
                && matches!(
                    ext.as_str(),
                    ".png"
                        | ".jpg"
                        | ".jpeg"
                        | ".gif"
                        | ".bmp"
                        | ".webp"
                        | ".heic"
                        | ".svg"
                        | ".tif"
                        | ".tiff"
                        | ".ico"
                        | ".psd"
                        | ".raw"
                        | ".avif"
                )
        }
        TypeCategory::Music => {
            !facts.is_folder
                && matches!(
                    ext.as_str(),
                    ".mp3" | ".flac" | ".wav" | ".m4a" | ".aac" | ".ogg" | ".wma" | ".opus"
                )
        }
        TypeCategory::Video => {
            !facts.is_folder
                && matches!(
                    ext.as_str(),
                    ".mp4" | ".mkv" | ".avi" | ".mov" | ".wmv" | ".webm" | ".m4v" | ".ts"
                )
        }
        TypeCategory::Archives => {
            !facts.is_folder
                && matches!(
                    ext.as_str(),
                    ".zip" | ".7z" | ".rar" | ".tar" | ".gz" | ".bz2" | ".xz" | ".iso" | ".cab"
                )
        }
        TypeCategory::Installers => {
            if facts.is_folder {
                return false;
            }
            match ext.as_str() {
                ".msi" | ".msix" | ".msixbundle" | ".appx" | ".appxbundle" | ".appinstaller" => {
                    true
                }
                ".exe" => {
                    let name = facts.file_name.to_lowercase();
                    name.contains("setup") || name.contains("install")
                }
                _ => false,
            }
        }
    }
}

fn str_op(op: StrOp, haystack: &str, needle: &str) -> bool {
    let h = haystack.to_lowercase();
    let n = needle.to_lowercase();
    match op {
        StrOp::Contains => h.contains(&n),
        StrOp::NotContains => !h.contains(&n),
        StrOp::StartsWith => h.starts_with(&n),
        StrOp::EndsWith => h.ends_with(&n),
        StrOp::Is => h == n,
    }
}

/// 极简 glob：`*` 与 `?`，忽略大小写。
pub fn glob_match(pattern: &str, text: &str) -> bool {
    fn rec(p: &[char], t: &[char]) -> bool {
        match (p.first(), t.first()) {
            (None, None) => true,
            (Some('*'), _) => rec(&p[1..], t) || (!t.is_empty() && rec(p, &t[1..])),
            (Some('?'), Some(_)) => rec(&p[1..], &t[1..]),
            (Some(a), Some(b)) if a.eq_ignore_ascii_case(b) => rec(&p[1..], &t[1..]),
            _ => false,
        }
    }
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    rec(&p, &t)
}

pub fn cond_matches(cond: &Cond, facts: &ItemFacts) -> bool {
    match cond {
        Cond::Type(cats) => cats.iter().any(|c| category_matches(*c, facts)),
        Cond::Ext(exts) => {
            // 后缀匹配，因此 `tar.gz` 与 `.gitignore` 这类点文件也能匹配。
            let name = facts.file_name.to_lowercase();
            exts.iter().any(|e| {
                let e = e.to_lowercase();
                let e = if e.starts_with('.') {
                    e
                } else {
                    format!(".{e}")
                };
                name.ends_with(&e)
            })
        }
        Cond::ExactName(names) => names
            .iter()
            .any(|n| n.eq_ignore_ascii_case(&facts.file_name)),
        Cond::Name { op, value } => str_op(*op, &facts.file_name, value),
        Cond::Glob(g) => glob_match(g, &facts.file_name),
        Cond::ShortcutTarget { op, value } => facts
            .shortcut_target
            .as_deref()
            .map(|t| str_op(*op, t, value))
            .unwrap_or(false),
        Cond::FoldersOnly => facts.is_folder,
        Cond::FilesOnly => !facts.is_folder,
        Cond::PlainFiles => {
            !facts.is_folder
                && facts.origin != Origin::Namespace
                && !category_matches(TypeCategory::Programs, facts)
                && !category_matches(TypeCategory::Shortcuts, facts)
        }
        Cond::SizeMb { min, max } => {
            let mb = facts.size_bytes as f64 / (1024.0 * 1024.0);
            min.is_none_or(|m| mb >= m) && max.is_none_or(|m| mb <= m)
        }
        Cond::CreatedTime { from_min, to_min } => match facts.created_minutes_local {
            Some(m) => {
                if from_min <= to_min {
                    m >= *from_min && m <= *to_min
                } else {
                    m >= *from_min || m <= *to_min
                }
            }
            None => false,
        },
        Cond::CreatedWeekday(days) => facts
            .created_weekday
            .map(|d| days.contains(&d))
            .unwrap_or(false),
        Cond::Origin(o) => facts.origin == *o,
        Cond::IdleDays { min } => facts.idle_days.is_some_and(|d| d >= *min),
    }
}

/// 对单个条目求值的结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// 临时/系统条目：暂不处理。
    Skip,
    /// 命中 `rule`，去它的目标。
    Route { target: Target, rule: RuleId },
    /// 无规则匹配：去默认目标。
    Default(Target),
}

impl RuleSet {
    pub fn evaluate(&self, facts: &ItemFacts) -> Decision {
        if is_temporary(&facts.file_name, facts.age_secs)
            || facts.is_system
            || facts.file_name.eq_ignore_ascii_case("desktop.ini")
        {
            return Decision::Skip;
        }
        let mut ordered: Vec<&Rule> = self.list.iter().filter(|r| r.enabled).collect();
        // 稳定排序：Target 类优先，其余保持用户顺序。
        ordered.sort_by_key(|r| {
            if r.priority_class == Class::Target {
                0
            } else {
                1
            }
        });
        for rule in ordered {
            if !rule.all_of.is_empty() && rule.all_of.iter().all(|c| cond_matches(c, facts)) {
                return Decision::Route {
                    target: rule.target,
                    rule: rule.id,
                };
            }
        }
        Decision::Default(self.default_target)
    }

    /// 是否有启用的规则依赖时钟（闲置天数），需要定期重跑而非只在桌面变化时跑。
    pub fn has_idle_rules(&self) -> bool {
        self.list
            .iter()
            .any(|r| r.enabled && r.all_of.iter().any(|c| matches!(c, Cond::IdleDays { .. })))
    }

    /// 首次运行预设：程序 / 文件夹 / 文件与文档。
    pub fn default_presets(programs: FenceId, folders: FenceId, documents: FenceId) -> Self {
        Self {
            default_target: Target::Inbox,
            keep_updated: true,
            unmatched: Unmatched::DefaultTarget,
            list: vec![
                Rule::new(
                    "程序与快捷方式",
                    Target::Fence(programs),
                    vec![Cond::Type(vec![
                        TypeCategory::Programs,
                        TypeCategory::Shortcuts,
                    ])],
                ),
                Rule::new(
                    "文件夹",
                    Target::Fence(folders),
                    vec![Cond::Type(vec![TypeCategory::Folders])],
                ),
                Rule::new(
                    "文件与文档",
                    Target::Fence(documents),
                    vec![Cond::Type(vec![
                        TypeCategory::Documents,
                        TypeCategory::Images,
                        TypeCategory::Music,
                        TypeCategory::Video,
                        TypeCategory::Archives,
                    ])],
                ),
            ],
        }
    }
}

/// 「待清理」模板的闲置阈值：安装包与压缩包超过这么久未动则聚拢（绝不删除）。
pub const CLEANUP_IDLE_DAYS: u32 = 30;

/// 「快速添加」预设：一键创建栅栏并生成填充它的规则。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Template {
    Images,
    Music,
    Video,
    Archives,
    Installers,
    Documents,
    Programs,
    Folders,
    Downloads,
    Cleanup,
}

impl Template {
    pub fn key(self) -> &'static str {
        match self {
            Template::Images => "images",
            Template::Music => "music",
            Template::Video => "video",
            Template::Archives => "archives",
            Template::Installers => "installers",
            Template::Documents => "documents",
            Template::Programs => "programs",
            Template::Folders => "folders",
            Template::Downloads => "downloads",
            Template::Cleanup => "cleanup",
        }
    }

    pub fn parse(key: &str) -> Option<Self> {
        Some(match key {
            "images" => Template::Images,
            "music" => Template::Music,
            "video" => Template::Video,
            "archives" => Template::Archives,
            "installers" => Template::Installers,
            "documents" => Template::Documents,
            "programs" => Template::Programs,
            "folders" => Template::Folders,
            "downloads" => Template::Downloads,
            "cleanup" => Template::Cleanup,
            _ => return None,
        })
    }

    pub fn title(self) -> &'static str {
        match self {
            Template::Images => "图片",
            Template::Music => "音乐",
            Template::Video => "视频",
            Template::Archives => "压缩包",
            Template::Installers => "安装包",
            Template::Documents => "文档",
            Template::Programs => "程序",
            Template::Folders => "文件夹",
            Template::Downloads => "下载",
            Template::Cleanup => "待清理",
        }
    }

    pub fn rule(self, fence: FenceId) -> Rule {
        let mut rule = match self {
            Template::Images => Rule::new(
                "图片",
                Target::Fence(fence),
                vec![Cond::Type(vec![TypeCategory::Images])],
            ),
            Template::Music => Rule::new(
                "音乐",
                Target::Fence(fence),
                vec![Cond::Type(vec![TypeCategory::Music])],
            ),
            Template::Video => Rule::new(
                "视频",
                Target::Fence(fence),
                vec![Cond::Type(vec![TypeCategory::Video])],
            ),
            Template::Archives => Rule::new(
                "压缩包",
                Target::Fence(fence),
                vec![Cond::Type(vec![TypeCategory::Archives])],
            ),
            Template::Installers => Rule::new(
                "安装包",
                Target::Fence(fence),
                vec![Cond::Type(vec![TypeCategory::Installers])],
            ),
            Template::Documents => Rule::new(
                "文档",
                Target::Fence(fence),
                vec![Cond::Type(vec![TypeCategory::Documents])],
            ),
            Template::Programs => Rule::new(
                "程序",
                Target::Fence(fence),
                vec![Cond::Type(vec![
                    TypeCategory::Programs,
                    TypeCategory::Shortcuts,
                ])],
            ),
            Template::Folders => Rule::new(
                "文件夹",
                Target::Fence(fence),
                vec![Cond::Type(vec![TypeCategory::Folders])],
            ),
            Template::Downloads => Rule::new(
                "下载",
                Target::Fence(fence),
                vec![Cond::Name {
                    op: StrOp::Contains,
                    value: "下载".into(),
                }],
            ),
            Template::Cleanup => Rule::new(
                "待清理",
                Target::Fence(fence),
                vec![
                    Cond::Type(vec![TypeCategory::Installers, TypeCategory::Archives]),
                    Cond::IdleDays {
                        min: CLEANUP_IDLE_DAYS,
                    },
                ],
            ),
        };
        rule.template = Some(self.key().to_string());
        rule
    }

    /// 全部模板，供前端「快速添加」列表使用。
    pub fn all() -> &'static [Template] {
        &[
            Template::Programs,
            Template::Folders,
            Template::Documents,
            Template::Images,
            Template::Music,
            Template::Video,
            Template::Archives,
            Template::Installers,
            Template::Downloads,
            Template::Cleanup,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(name: &str) -> ItemFacts {
        ItemFacts {
            file_name: name.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn ext_of_handles_dotfiles_and_no_ext() {
        assert_eq!(ext_of("Report.DOCX"), ".docx");
        assert_eq!(ext_of("README"), "");
        // 前导点视为点文件，不算扩展名
        assert_eq!(ext_of(".gitignore"), "");
    }

    #[test]
    fn temporary_files_are_skipped() {
        assert!(is_temporary("~$Report.docx", Some(9999)));
        assert!(is_temporary("movie.mp4.crdownload", Some(9999)));
        // .tmp 需要沉淀时间
        assert!(is_temporary("save.tmp", Some(10)));
        assert!(!is_temporary("save.tmp", Some(TMP_SETTLE_SECS)));
        assert!(!is_temporary("Report.docx", Some(1)));
    }

    #[test]
    fn classify_orders_folders_before_files() {
        let mut f = facts("Photos");
        f.is_folder = true;
        assert_eq!(classify(&f), Some(TypeCategory::Folders));
        assert_eq!(classify(&facts("a.zip")), Some(TypeCategory::Archives));
        assert_eq!(classify(&facts("a.png")), Some(TypeCategory::Images));
        assert_eq!(classify(&facts("a.unknownext")), None);
    }

    #[test]
    fn lnk_to_program_counts_as_program() {
        let mut f = facts("Steam.lnk");
        f.shortcut_target_ext = Some(".exe".into());
        assert!(category_matches(TypeCategory::Programs, &f));
        // 无目标信息的 .lnk 也归为程序
        let plain = facts("Steam.lnk");
        assert!(category_matches(TypeCategory::Programs, &plain));
    }

    #[test]
    fn setup_exe_is_installer() {
        assert!(category_matches(
            TypeCategory::Installers,
            &facts("SteamSetup.exe")
        ));
        assert!(!category_matches(
            TypeCategory::Installers,
            &facts("game.exe")
        ));
        assert!(category_matches(
            TypeCategory::Installers,
            &facts("pkg.msix")
        ));
    }

    #[test]
    fn glob_matches_wildcards() {
        assert!(glob_match("*.png", "shot.PNG"));
        assert!(glob_match("report?.docx", "report1.docx"));
        assert!(!glob_match("*.png", "shot.jpg"));
        assert!(glob_match("*", "anything"));
    }

    #[test]
    fn ext_condition_matches_suffix_including_compound() {
        let f = facts("backup.tar.gz");
        assert!(cond_matches(&Cond::Ext(vec!["tar.gz".into()]), &f));
        assert!(cond_matches(&Cond::Ext(vec![".gz".into()]), &f));
        assert!(!cond_matches(&Cond::Ext(vec!["zip".into()]), &f));
    }

    #[test]
    fn size_condition_bounds() {
        let mut f = facts("big.iso");
        f.size_bytes = 500 * 1024 * 1024;
        assert!(cond_matches(
            &Cond::SizeMb {
                min: Some(100.0),
                max: None
            },
            &f
        ));
        assert!(!cond_matches(
            &Cond::SizeMb {
                min: Some(600.0),
                max: None
            },
            &f
        ));
        assert!(cond_matches(
            &Cond::SizeMb {
                min: Some(100.0),
                max: Some(1000.0)
            },
            &f
        ));
    }

    #[test]
    fn created_time_wraps_past_midnight() {
        let mut f = facts("late.txt");
        f.created_minutes_local = Some(30); // 00:30
        assert!(cond_matches(
            &Cond::CreatedTime {
                from_min: 22 * 60,
                to_min: 6 * 60
            },
            &f
        ));
        assert!(!cond_matches(
            &Cond::CreatedTime {
                from_min: 8 * 60,
                to_min: 18 * 60
            },
            &f
        ));
    }

    #[test]
    fn idle_days_never_matches_when_unknown() {
        let f = facts("x.zip");
        assert!(!cond_matches(&Cond::IdleDays { min: 1 }, &f));
        let mut known = f.clone();
        known.idle_days = Some(45);
        assert!(cond_matches(&Cond::IdleDays { min: 30 }, &known));
        assert!(!cond_matches(&Cond::IdleDays { min: 60 }, &known));
    }

    #[test]
    fn evaluate_skips_desktop_ini_and_system() {
        let rs = RuleSet::default();
        assert_eq!(rs.evaluate(&facts("desktop.ini")), Decision::Skip);
        let mut sys = facts("a.txt");
        sys.is_system = true;
        assert_eq!(rs.evaluate(&sys), Decision::Skip);
    }

    #[test]
    fn evaluate_routes_to_first_matching_rule() {
        let fence = Uuid::new_v4();
        let rs = RuleSet {
            default_target: Target::Inbox,
            keep_updated: true,
            unmatched: Unmatched::DefaultTarget,
            list: vec![Rule::new(
                "图片",
                Target::Fence(fence),
                vec![Cond::Type(vec![TypeCategory::Images])],
            )],
        };
        match rs.evaluate(&facts("a.png")) {
            Decision::Route { target, .. } => assert_eq!(target, Target::Fence(fence)),
            other => panic!("expected Route, got {other:?}"),
        }
        assert_eq!(
            rs.evaluate(&facts("a.txt")),
            Decision::Default(Target::Inbox)
        );
    }

    #[test]
    fn disabled_rules_are_ignored() {
        let fence = Uuid::new_v4();
        let mut rule = Rule::new(
            "图片",
            Target::Fence(fence),
            vec![Cond::Type(vec![TypeCategory::Images])],
        );
        rule.enabled = false;
        let rs = RuleSet {
            list: vec![rule],
            ..Default::default()
        };
        assert_eq!(
            rs.evaluate(&facts("a.png")),
            Decision::Default(Target::Inbox)
        );
    }

    #[test]
    fn rule_with_no_conditions_never_matches() {
        let rs = RuleSet {
            list: vec![Rule::new("空", Target::Inbox, vec![])],
            ..Default::default()
        };
        assert_eq!(
            rs.evaluate(&facts("a.txt")),
            Decision::Default(Target::Inbox)
        );
    }

    #[test]
    fn target_class_rules_evaluate_first() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        // 普通类型规则在前，Target 类规则在后，但 Target 类应先命中
        let type_rule = Rule::new(
            "图片",
            Target::Fence(a),
            vec![Cond::Type(vec![TypeCategory::Images])],
        );
        let mut target_rule = Rule::new(
            "快捷方式指向图片",
            Target::Fence(b),
            vec![Cond::ShortcutTarget {
                op: StrOp::EndsWith,
                value: ".png".into(),
            }],
        );
        target_rule.priority_class = Class::Target;
        let rs = RuleSet {
            list: vec![type_rule, target_rule],
            ..Default::default()
        };
        let mut f = facts("shot.png");
        f.shortcut_target = Some("C:\\pics\\x.png".into());
        match rs.evaluate(&f) {
            Decision::Route { target, .. } => assert_eq!(target, Target::Fence(b)),
            other => panic!("expected Route to target-class rule, got {other:?}"),
        }
    }

    #[test]
    fn has_idle_rules_detects_clock_dependent_rules() {
        let mut rs = RuleSet::default();
        assert!(!rs.has_idle_rules());
        rs.list.push(Rule::new(
            "待清理",
            Target::Inbox,
            vec![Cond::IdleDays { min: 30 }],
        ));
        assert!(rs.has_idle_rules());
    }

    #[test]
    fn templates_roundtrip_by_key() {
        for t in Template::all() {
            assert_eq!(Template::parse(t.key()), Some(*t));
        }
        assert_eq!(Template::parse("nope"), None);
    }

    #[test]
    fn cleanup_template_requires_idle_and_type() {
        let fence = Uuid::new_v4();
        let rule = Template::Cleanup.rule(fence);
        assert_eq!(rule.all_of.len(), 2);
        assert_eq!(rule.template.as_deref(), Some("cleanup"));
    }

    #[test]
    fn ruleset_roundtrips_through_json() {
        let fence = Uuid::new_v4();
        let rs = RuleSet::default_presets(fence, Uuid::new_v4(), Uuid::new_v4());
        let json = serde_json::to_string(&rs).unwrap();
        let back: RuleSet = serde_json::from_str(&json).unwrap();
        assert_eq!(back.list.len(), 3);
        assert_eq!(back.list[0].name, "程序与快捷方式");
    }
}
