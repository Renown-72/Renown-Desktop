//! 桌面栅栏（桌面整理）模块。
//!
//! 设计移植自 [PecoFence](https://github.com/DayuanJiang/PecoFence)（Apache-2.0）：
//! 不操作 `SysListView32` 里的图标项，而是直接读桌面文件夹的真实文件、
//! 隐藏原生图标层，再用自己的窗口绘制栅栏。
//!
//! 模块划分：
//!
//! | 模块 | 职责 |
//! |---|---|
//! | [`model`] | 纯数据结构（栅栏、条目、布局、几何） |
//! | [`rules`] | 自动归类规则引擎（纯函数） |
//! | [`store`] | 配置持久化（原子写） |
//! | [`desktop`] | 桌面文件夹枚举与条目合并 |
//! | [`icons`] | shell 图标提取与缓存 |
//! | [`monitor`] | 显示器枚举与布局指纹匹配 |
//! | [`window`] | Win32 窗口层（隐藏原生图标、z 序） |
//! | [`shortcut`] | `.lnk` / `.url` 目标解析 |
//! | [`service`] | 业务逻辑（串行化修改） |
//! | [`windows`] | 栅栏窗口生命周期 |
//! | [`commands`] | Tauri 命令 |

pub mod commands;
pub mod desktop;
pub mod icons;
pub mod model;
pub mod monitor;
pub mod rules;
pub mod service;
pub mod shell_paths;
pub mod shortcut;
pub mod store;
pub mod window;
pub mod windows;

pub use service::FenceService;

/// 全局 AppHandle，供命令层广播配置变更事件。
static APP_HANDLE: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

/// 配置变更事件名。
pub const CHANGED_EVENT: &str = "fence-changed";

/// 广播「栅栏配置已变更」，让所有栅栏窗口刷新。
pub fn notify_changed() {
    use tauri::Emitter;
    if let Some(app) = APP_HANDLE.get() {
        let _ = app.emit(CHANGED_EVENT, ());
    }
}

/// 创建栅栏服务并注册为 Tauri 托管状态。
pub fn init(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::Manager;
    let service = std::sync::Arc::new(FenceService::new());
    app.manage(service);
    let _ = APP_HANDLE.set(app.handle().clone());
    Ok(())
}

/// 启动后的初始化：同步桌面、按配置隐藏原生图标。
///
/// 放在独立线程里跑，避免阻塞窗口显示（枚举桌面 + 提取图标可能耗时）。
pub fn startup_sync(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        use tauri::Manager;
        let Some(service) = app.try_state::<std::sync::Arc<FenceService>>() else {
            return;
        };
        let report = service.sync_desktop();
        let _ = service.save();
        eprintln!(
            "[fence] 桌面同步完成: 新增 {} / 消失 {} / 重新归类 {} / 总计 {}",
            report.added, report.removed, report.rerouted, report.total
        );
        let hide = service.snapshot().settings.hide_real_icons;
        commands::apply_icon_visibility(hide);
        // 建出栅栏窗口
        let layout = service.active_layout();
        windows::sync_layout(&app, &layout);
    });
}

/// 退出前的清理：恢复原生桌面图标显示。
pub fn shutdown(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(service) = app.try_state::<std::sync::Arc<FenceService>>() {
        let _ = service.save();
    }
    windows::close_all(app);
    // 无论配置如何，退出时都把原生图标还回去，避免用户桌面「空了」。
    if let Some(host) = window::resolve_icon_host() {
        window::set_icons_visible(&host, true);
    }
}
