#![windows_subsystem = "windows"]

use native_windows_gui as nwg;
use native_windows_derive as nwd;

use nwd::NwgUi;
use nwg::NativeUi;
use std::os::windows::ffi::OsStrExt;
use std::cell::Cell;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::path::{Path, PathBuf};
use winreg::enums::*;
use winreg::RegKey;
use serde::Deserialize;
use std::os::windows::process::CommandExt;

const EM_SETCUEBANNER: u32 = 0x1501;

static CONFIG_STR: &str = include_str!(concat!(env!("OUT_DIR"), "/config.toml"));
static PAYLOAD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload.tar.zst"));
static POST_INSTALL_HOOK: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/post_install.bin"));
static PRE_UNINSTALL_HOOK: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/pre_uninstall.bin"));

#[derive(Deserialize, Clone)]
pub struct Config {
    pub app: AppConfig,
    pub install: InstallConfig,
    pub build: Option<BuildConfig>,
    pub hooks: Option<HooksConfig>,
}

#[derive(Deserialize, Clone)]
pub struct AppConfig {
    pub name: String,
    pub version: String,
    pub publisher: String,
    pub main_exe: String,
    pub icon: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct InstallConfig {
    pub source_dir: String,
    pub default_dir: String,
    pub create_desktop_shortcut: Option<bool>,
    pub create_start_menu_shortcut: Option<bool>,
}

#[derive(Deserialize, Clone)]
pub struct BuildConfig {
    pub output_name: Option<String>,
    pub compression_level: Option<i32>,
}

#[derive(Deserialize, Clone)]
pub struct HooksConfig {
    pub post_install: Option<String>,
    pub pre_uninstall: Option<String>,
}

fn get_config() -> Config {
    toml::from_str(CONFIG_STR).expect("config parse error")
}

#[derive(Default, NwgUi)]
pub struct InstallerApp {
    #[nwg_control(size: (410, 116), position: (350, 300), title: "", flags: "WINDOW")]
    #[nwg_events( OnWindowClose: [InstallerApp::on_cancel_or_exit] )]
    window: nwg::Window,

    #[nwg_resource(title: "Select install directory", action: nwg::FileDialogAction::OpenDirectory)]
    dir_dialog: nwg::FileDialog,

    #[nwg_resource(family: "Segoe UI", size: 15)]
    ui_font: nwg::Font,

    #[nwg_control(text: "", size: (344, 24), position: (14, 14), font: Some(&data.ui_font))]
    txt_path: nwg::TextInput,

    #[nwg_control(text: "...", size: (34, 24), position: (362, 14), font: Some(&data.ui_font))]
    #[nwg_events( OnButtonClick: [InstallerApp::on_browse] )]
    btn_browse: nwg::Button,

    #[nwg_control(text: "Desktop shortcut", size: (170, 20), position: (14, 44), check_state: nwg::CheckBoxState::Checked, font: Some(&data.ui_font))]
    chk_desktop: nwg::CheckBox,

    #[nwg_control(text: "Start menu shortcut", size: (126, 20), position: (232, 44), check_state: nwg::CheckBoxState::Checked, font: Some(&data.ui_font))]
    chk_start_menu: nwg::CheckBox,

    #[nwg_control(text: "Installing...", size: (382, 20), position: (14, 18), font: Some(&data.ui_font))]
    lbl_status: nwg::Label,

    #[nwg_control(range: 0..100, pos: 0, size: (382, 20), position: (14, 44))]
    progress_bar: nwg::ProgressBar,

    #[nwg_control]
    #[nwg_events( OnNotice: [InstallerApp::on_progress_update] )]
    progress_notice: nwg::Notice,

    #[nwg_control(text: "Install", size: (187, 28), position: (14, 74), font: Some(&data.ui_font))]
    #[nwg_events( OnButtonClick: [InstallerApp::on_install_or_launch] )]
    btn_install: nwg::Button,

    #[nwg_control(text: "Cancel", size: (187, 28), position: (209, 74), font: Some(&data.ui_font))]
    #[nwg_events( OnButtonClick: [InstallerApp::on_cancel_or_exit] )]
    btn_cancel: nwg::Button,

    stage: Cell<u8>,
    cancel_flag: Cell<Option<Arc<AtomicBool>>>,
}

static PROGRESS_VALUE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static PROGRESS_STATUS_ID: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

impl InstallerApp {
    fn on_browse(&self) {
        if self.dir_dialog.run(Some(&self.window)) {
            if let Ok(dir) = self.dir_dialog.get_selected_item() {
                if let Some(dir_str) = dir.to_str() {
                    self.txt_path.set_text(dir_str);
                }
            }
        }
    }

    fn on_install_or_launch(&self) {
        let cfg = get_config();
        if self.stage.get() == 2 {
            let install_dir = self.txt_path.text();
            let exe_path = format!("{}\\{}", install_dir.trim_end_matches('\\'), cfg.app.main_exe);
            let _ = std::process::Command::new(&exe_path).spawn();
            self.save_window_position();
            nwg::stop_thread_dispatch();
            return;
        }

        let path = self.txt_path.text();
        if path.trim().is_empty() {
            nwg::simple_message("Error", "Specify destination path");
            return;
        }

        self.stage.set(1);

        self.txt_path.set_visible(false);
        self.btn_browse.set_visible(false);
        self.chk_desktop.set_visible(false);
        self.chk_start_menu.set_visible(false);

        self.lbl_status.set_text("Preparing...");
        self.lbl_status.set_visible(true);
        self.progress_bar.set_visible(true);
        self.progress_bar.set_pos(0);

        self.btn_install.set_enabled(false);
        self.btn_install.set_text("Installing...");
        self.btn_cancel.set_text("Cancel");

        let install_path = PathBuf::from(path.trim());
        let create_desktop = self.chk_desktop.check_state() == nwg::CheckBoxState::Checked;
        let create_start = self.chk_start_menu.check_state() == nwg::CheckBoxState::Checked;

        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.cancel_flag.set(Some(cancel_flag.clone()));

        let notice_sender = self.progress_notice.sender();

        std::thread::spawn(move || {
            perform_installation(cfg, install_path, create_desktop, create_start, cancel_flag, notice_sender);
        });
    }

    fn on_progress_update(&self) {
        let val = PROGRESS_VALUE.load(Ordering::SeqCst);
        let status_id = PROGRESS_STATUS_ID.load(Ordering::SeqCst);

        self.progress_bar.set_pos(val);

        match status_id {
            1 => self.lbl_status.set_text("Extracting files..."),
            2 => self.lbl_status.set_text("Creating shortcuts..."),
            3 => self.lbl_status.set_text("Configuring environment..."),
            4 => {
                self.stage.set(2);
                self.progress_bar.set_pos(100);
                self.lbl_status.set_text("Completed successfully");
                self.btn_install.set_text("Launch");
                self.btn_install.set_enabled(true);
                self.btn_cancel.set_text("Close");
            }
            5 => {
                self.lbl_status.set_text("Cleaning up...");
                self.btn_install.set_enabled(false);
                self.btn_cancel.set_enabled(false);
            }
            _ => {}
        }
    }

    fn on_cancel_or_exit(&self) {
        if self.stage.get() == 1 {
            if let Some(flag) = self.cancel_flag.take() {
                flag.store(true, Ordering::SeqCst);
            }
            self.lbl_status.set_text("Cancelling...");
            self.btn_install.set_enabled(false);
            self.btn_cancel.set_enabled(false);

            let install_dir = self.txt_path.text();
            let dir_path = std::path::Path::new(&install_dir);
            if dir_path.exists() {
                let _ = std::fs::remove_dir_all(dir_path);
            }

            self.save_window_position();
            nwg::stop_thread_dispatch();
            return;
        }

        self.save_window_position();
        nwg::stop_thread_dispatch();
    }

    fn save_window_position(&self) {
        let (x, y) = self.window.position();
        save_position_to_cache(x, y);
    }
}

fn perform_installation(
    cfg: Config,
    target_dir: PathBuf,
    create_desktop: bool,
    create_start: bool,
    cancel_flag: Arc<AtomicBool>,
    notice: nwg::NoticeSender,
) {
    if let Err(_) = std::fs::create_dir_all(&target_dir) {
        return;
    }

    PROGRESS_STATUS_ID.store(1, Ordering::SeqCst);
    PROGRESS_VALUE.store(10, Ordering::SeqCst);
    notice.notice();

    let decoder = match zstd::stream::Decoder::new(PAYLOAD) {
        Ok(d) => d,
        Err(_) => return,
    };
    let mut archive = tar::Archive::new(decoder);

    if let Ok(entries) = archive.entries() {
        let mut count = 0;
        for entry in entries {
            if cancel_flag.load(Ordering::SeqCst) {
                let _ = std::fs::remove_dir_all(&target_dir);
                return;
            }

            if let Ok(mut file) = entry {
                let _ = file.unpack_in(&target_dir);
                count += 1;
                let progress = 10 + (count * 20).min(65);
                PROGRESS_VALUE.store(progress, Ordering::SeqCst);
                notice.notice();
            }
        }
    }

    if cancel_flag.load(Ordering::SeqCst) {
        let _ = std::fs::remove_dir_all(&target_dir);
        return;
    }

    let uninstaller_path = target_dir.join("unins000.exe");
    if let Ok(current_exe) = std::env::current_exe() {
        let _ = std::fs::copy(current_exe, &uninstaller_path);
    }

    PROGRESS_STATUS_ID.store(2, Ordering::SeqCst);
    PROGRESS_VALUE.store(80, Ordering::SeqCst);
    notice.notice();

    let exe_path = target_dir.join(&cfg.app.main_exe);
    let lnk_name = format!("{}.lnk", cfg.app.name);

    if create_desktop {
        if let Some(desktop_dir) = get_special_folder(winapi::um::shlobj::CSIDL_DESKTOPDIRECTORY) {
            create_shortcut(&exe_path, &desktop_dir.join(&lnk_name), &cfg.app.name);
        }
    }

    if create_start {
        if let Some(programs_dir) = get_special_folder(winapi::um::shlobj::CSIDL_PROGRAMS) {
            create_shortcut(&exe_path, &programs_dir.join(&lnk_name), &cfg.app.name);
        }
    }

    PROGRESS_STATUS_ID.store(3, Ordering::SeqCst);
    PROGRESS_VALUE.store(90, Ordering::SeqCst);
    notice.notice();

    register_app(&cfg, &target_dir, &exe_path, &uninstaller_path);

    if !POST_INSTALL_HOOK.is_empty() {
        run_hook(POST_INSTALL_HOOK, &target_dir, &exe_path);
    }

    PROGRESS_STATUS_ID.store(4, Ordering::SeqCst);
    PROGRESS_VALUE.store(100, Ordering::SeqCst);
    notice.notice();
}

fn run_hook(hook_bytes: &[u8], target_dir: &Path, exe_path: &Path) {
    let temp_script = std::env::temp_dir().join(format!("drop_hook_{}.bat", std::process::id()));
    if std::fs::write(&temp_script, hook_bytes).is_ok() {
        let _ = std::process::Command::new("cmd")
            .args(["/c", &temp_script.to_string_lossy(), &target_dir.to_string_lossy(), &exe_path.to_string_lossy()])
            .creation_flags(0x08000000)
            .status();
        let _ = std::fs::remove_file(&temp_script);
    }
}

fn register_app(cfg: &Config, target_dir: &Path, exe_path: &Path, uninstaller_path: &Path) {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let subkey_name = format!(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{}", cfg.app.name.replace(' ', ""));
    if let Ok((uninst_key, _)) = hkcu.create_subkey(&subkey_name) {
        let _ = uninst_key.set_value("DisplayName", &cfg.app.name);
        let _ = uninst_key.set_value("DisplayIcon", &format!("{},0", exe_path.to_string_lossy()));
        let _ = uninst_key.set_value("DisplayVersion", &cfg.app.version);
        let _ = uninst_key.set_value("Publisher", &cfg.app.publisher);
        let _ = uninst_key.set_value("InstallLocation", &target_dir.to_string_lossy().to_string());
        let _ = uninst_key.set_value("UninstallString", &format!("\"{}\" --uninstall", uninstaller_path.to_string_lossy()));
        let _ = uninst_key.set_value("QuietUninstallString", &format!("\"{}\" --uninstall", uninstaller_path.to_string_lossy()));
    }
}

fn perform_uninstall(cfg: Config) {
    let my_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_default();
    let exe_path = my_dir.join(&cfg.app.main_exe);

    if !PRE_UNINSTALL_HOOK.is_empty() {
        run_hook(PRE_UNINSTALL_HOOK, &my_dir, &exe_path);
    }

    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/IM", &cfg.app.main_exe])
        .creation_flags(0x08000000)
        .output();

    let lnk_name = format!("{}.lnk", cfg.app.name);
    if let Some(desktop_dir) = get_special_folder(winapi::um::shlobj::CSIDL_DESKTOPDIRECTORY) {
        let _ = std::fs::remove_file(desktop_dir.join(&lnk_name));
    }
    if let Some(programs_dir) = get_special_folder(winapi::um::shlobj::CSIDL_PROGRAMS) {
        let _ = std::fs::remove_file(programs_dir.join(&lnk_name));
    }

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let subkey_name = format!(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{}", cfg.app.name.replace(' ', ""));
    let _ = hkcu.delete_subkey_all(&subkey_name);

    if my_dir.exists() {
        let dir_str = my_dir.to_string_lossy();
        let cmd = format!("timeout /t 1 /nobreak > nul & rmdir /s /q \"{}\"", dir_str);
        let _ = std::process::Command::new("cmd")
            .args(["/c", &cmd])
            .creation_flags(0x08000000)
            .spawn();
    }

    std::process::exit(0);
}

fn create_shortcut(target: &Path, link_path: &Path, _description: &str) {
    let mut sl = mslnk::ShellLink::new(target).unwrap_or_else(|_| mslnk::ShellLink::new("").unwrap());
    if let Some(workdir) = target.parent() {
        sl.set_working_dir(Some(workdir.to_string_lossy().to_string()));
    }
    let _ = sl.create_lnk(link_path);
}

fn get_special_folder(csidl: i32) -> Option<PathBuf> {
    unsafe {
        let mut path: [u16; winapi::shared::minwindef::MAX_PATH] = [0; winapi::shared::minwindef::MAX_PATH];
        if winapi::um::shlobj::SHGetFolderPathW(
            std::ptr::null_mut(),
            csidl,
            std::ptr::null_mut(),
            0,
            path.as_mut_ptr(),
        ) == winapi::shared::winerror::S_OK {
            let len = path.iter().position(|&c| c == 0).unwrap_or(path.len());
            let s = String::from_utf16_lossy(&path[..len]);
            Some(PathBuf::from(s))
        } else {
            None
        }
    }
}

fn get_cache_file() -> Option<std::path::PathBuf> {
    std::env::var("TEMP").ok().map(|t| std::path::PathBuf::from(t).join("drop_installer_pos.txt"))
}

fn save_position_to_cache(x: i32, y: i32) {
    if let Some(path) = get_cache_file() {
        let _ = std::fs::write(path, format!("{},{}", x, y));
    }
}

fn load_position_from_cache() -> Option<(i32, i32)> {
    let path = get_cache_file()?;
    let content = std::fs::read_to_string(path).ok()?;
    let parts: Vec<&str> = content.trim().split(',').collect();
    if parts.len() == 2 {
        let x = parts[0].parse::<i32>().ok()?;
        let y = parts[1].parse::<i32>().ok()?;
        Some((x, y))
    } else {
        None
    }
}

fn set_cue_banner(control: &nwg::TextInput, placeholder: &str) {
    let wide: Vec<u16> = std::ffi::OsStr::new(placeholder)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        winapi::um::winuser::SendMessageW(
            control.handle.hwnd().unwrap() as _,
            EM_SETCUEBANNER,
            1,
            wide.as_ptr() as _,
        );
    }
}

fn expand_env_path(raw: &str) -> String {
    let mut res = raw.to_string();
    if let Ok(val) = std::env::var("APPDATA") {
        res = res.replace("%APPDATA%", &val);
    }
    if let Ok(val) = std::env::var("LOCALAPPDATA") {
        res = res.replace("%LOCALAPPDATA%", &val);
    }
    if let Ok(val) = std::env::var("ProgramFiles") {
        res = res.replace("%PROGRAMFILES%", &val);
    }
    res
}

fn center_or_restore_window(window: &nwg::Window) {
    if let Some((x, y)) = load_position_from_cache() {
        window.set_position(x, y);
    } else {
        unsafe {
            let screen_w = winapi::um::winuser::GetSystemMetrics(winapi::um::winuser::SM_CXSCREEN);
            let screen_h = winapi::um::winuser::GetSystemMetrics(winapi::um::winuser::SM_CYSCREEN);
            let win_w = 410;
            let win_h = 116;
            let x = (screen_w - win_w) / 2;
            let y = (screen_h - win_h) / 2;
            window.set_position(x, y);
        }
    }
    window.set_visible(true);
}

fn main() {
    let cfg = get_config();
    let args: Vec<String> = std::env::args().collect();
    let current_exe_name = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()))
        .unwrap_or_default();

    if args.iter().any(|a| a == "--uninstall") || current_exe_name.starts_with("unins") {
        perform_uninstall(cfg);
        return;
    }

    nwg::init().expect("ui init failed");

    let app = InstallerApp::build_ui(Default::default()).expect("ui build failed");
    
    app.window.set_text(&format!("Install {}", cfg.app.name));
    app.lbl_status.set_visible(false);
    app.progress_bar.set_visible(false);

    if let Some(desktop_default) = cfg.install.create_desktop_shortcut {
        app.chk_desktop.set_check_state(if desktop_default { nwg::CheckBoxState::Checked } else { nwg::CheckBoxState::Unchecked });
    }
    if let Some(start_default) = cfg.install.create_start_menu_shortcut {
        app.chk_start_menu.set_check_state(if start_default { nwg::CheckBoxState::Checked } else { nwg::CheckBoxState::Unchecked });
    }

    set_cue_banner(&app.txt_path, "Destination path");
    app.txt_path.set_text(&expand_env_path(&cfg.install.default_dir));

    center_or_restore_window(&app.window);

    nwg::dispatch_thread_events();
}

#[no_mangle]
pub unsafe extern "system" fn GetWindowSubclass(
    hwnd: winapi::shared::windef::HWND,
    proc: winapi::um::commctrl::SUBCLASSPROC,
    uid: winapi::shared::basetsd::UINT_PTR,
    data: *mut winapi::shared::basetsd::DWORD_PTR,
) -> winapi::shared::minwindef::BOOL {
    type FuncType = unsafe extern "system" fn(
        winapi::shared::windef::HWND,
        winapi::um::commctrl::SUBCLASSPROC,
        winapi::shared::basetsd::UINT_PTR,
        *mut winapi::shared::basetsd::DWORD_PTR,
    ) -> winapi::shared::minwindef::BOOL;

    let h_mod = winapi::um::libloaderapi::GetModuleHandleA(b"comctl32.dll\0".as_ptr() as _);
    if h_mod.is_null() {
        return 0;
    }

    let mut proc_addr = winapi::um::libloaderapi::GetProcAddress(h_mod, b"GetWindowSubclass\0".as_ptr() as _);
    if proc_addr.is_null() {
        proc_addr = winapi::um::libloaderapi::GetProcAddress(h_mod, 411 as *const _);
    }

    if let Some(func) = std::mem::transmute::<_, Option<FuncType>>(proc_addr) {
        func(hwnd, proc, uid, data)
    } else {
        0
    }
}
