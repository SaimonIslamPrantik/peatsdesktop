use serde::{Deserialize, Serialize};
use std::ffi::c_void;
use std::path::PathBuf;
use tray_icon::Icon;

#[derive(Debug, Clone, Serialize)]
pub struct SystemTheme {
    pub dark_mode: bool,
    pub accent_hex: String,
    pub accent_rgb: [u8; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_palette_idx")]
    pub palette_idx: i32,
}

fn default_palette_idx() -> i32 {
    -1
}

impl Default for AppConfig {
    fn default() -> Self {
        Self { palette_idx: -1 }
    }
}

fn config_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(|p| PathBuf::from(p).join("PeatsDesktop").join("config.json"))
}

pub fn load_config() -> AppConfig {
    let Some(path) = config_path() else {
        return AppConfig::default();
    };
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_config(palette_idx: i32) {
    let Some(path) = config_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let cfg = AppConfig { palette_idx };
    if let Ok(json) = serde_json::to_string_pretty(&cfg) {
        let _ = std::fs::write(path, json);
    }
}

#[repr(C)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

const HKEY_CURRENT_USER: isize = 0x80000001u32 as isize;
const RRF_RT_REG_SZ: u32 = 0x00000002;
const RRF_RT_REG_DWORD: u32 = 0x00000010;
const KEY_WRITE: u32 = 0x00020006;
const REG_SZ: u32 = 1;
const RUN_SUBKEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const STARTUP_VALUE_NAME: &str = "PeatsDesktop";

const SPI_GETWORKAREA: u32 = 0x0030;
const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
const DWMWCP_ROUND: u32 = 2;

#[link(name = "advapi32")]
extern "system" {
    fn RegGetValueW(
        hkey: isize,
        lpsubkey: *const u16,
        lpvalue: *const u16,
        dwflags: u32,
        pdwtype: *mut u32,
        pvdata: *mut c_void,
        pcbdata: *mut u32,
    ) -> i32;
    fn RegOpenKeyExW(
        hkey: isize,
        lpsubkey: *const u16,
        uloptions: u32,
        samdesired: u32,
        phkresult: *mut isize,
    ) -> i32;
    fn RegSetValueExW(
        hkey: isize,
        lpvaluename: *const u16,
        reserved: u32,
        dwtype: u32,
        lpdata: *const u8,
        cbdata: u32,
    ) -> i32;
    fn RegDeleteValueW(hkey: isize, lpvaluename: *const u16) -> i32;
    fn RegCloseKey(hkey: isize) -> i32;
}

#[link(name = "user32")]
extern "system" {
    fn SystemParametersInfoW(
        uiaction: u32,
        uiparam: u32,
        pvparam: *mut c_void,
        fwinini: u32,
    ) -> i32;
}

#[link(name = "dwmapi")]
extern "system" {
    fn DwmSetWindowAttribute(
        hwnd: isize,
        dwattribute: u32,
        pvattribute: *const c_void,
        cbattribute: u32,
    ) -> i32;
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn is_startup_enabled() -> bool {
    let sub = to_wide(RUN_SUBKEY);
    let val = to_wide(STARTUP_VALUE_NAME);
    let mut size: u32 = 0;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            sub.as_ptr(),
            val.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut size,
        )
    };
    status == 0 && size > 2
}

pub fn set_startup_enabled(enable: bool) -> bool {
    let sub = to_wide(RUN_SUBKEY);
    let val = to_wide(STARTUP_VALUE_NAME);
    let mut hkey: isize = 0;
    let opened = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            sub.as_ptr(),
            0,
            KEY_WRITE,
            &mut hkey,
        )
    };
    if opened != 0 {
        return false;
    }

    let ok = if enable {
        if let Ok(exe) = std::env::current_exe() {
            let cmd = format!("\"{}\"", exe.display());
            let wide = to_wide(&cmd);
            let bytes_len = (wide.len() * std::mem::size_of::<u16>()) as u32;
            unsafe {
                RegSetValueExW(
                    hkey,
                    val.as_ptr(),
                    0,
                    REG_SZ,
                    wide.as_ptr() as *const u8,
                    bytes_len,
                ) == 0
            }
        } else {
            false
        }
    } else {
        unsafe { RegDeleteValueW(hkey, val.as_ptr()) == 0 }
    };

    unsafe {
        RegCloseKey(hkey);
    }
    ok
}

fn reg_read_dword(subkey: &str, value_name: &str) -> Option<u32> {
    let sub = to_wide(subkey);
    let val = to_wide(value_name);
    let mut out: u32 = 0;
    let mut size: u32 = 4;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            sub.as_ptr(),
            val.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            &mut out as *mut u32 as *mut c_void,
            &mut size,
        )
    };
    if status == 0 {
        Some(out)
    } else {
        None
    }
}

pub fn read_system_theme() -> SystemTheme {
    let dark_mode = reg_read_dword(
        "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
        "AppsUseLightTheme",
    )
    .map(|v| v == 0)
    .unwrap_or(true);

    let abgr = reg_read_dword("Software\\Microsoft\\Windows\\DWM", "AccentColor")
        .unwrap_or(0xFFFE7A68);
    let r = (abgr & 0xFF) as u8;
    let g = ((abgr >> 8) & 0xFF) as u8;
    let b = ((abgr >> 16) & 0xFF) as u8;

    SystemTheme {
        dark_mode,
        accent_hex: format!("#{:02X}{:02X}{:02X}", r, g, b),
        accent_rgb: [r, g, b],
    }
}

pub fn apply_dwm_flyout_style(hwnd: isize, dark: bool) {
    unsafe {
        let corner = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const u32 as *const c_void,
            4,
        );
        let dark_val: i32 = if dark { 1 } else { 0 };
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark_val as *const i32 as *const c_void,
            4,
        );
    }
}

pub fn get_work_area() -> Rect {
    let mut rect = Rect {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1032,
    };
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            &mut rect as *mut Rect as *mut c_void,
            0,
        );
    }
    rect
}

pub fn build_tray_icon(accent: [u8; 3], connected: bool, anc_mode: Option<u8>) -> Icon {
    let size = 32usize;
    let mut rgba = vec![0u8; size * size * 4];

    let dot_color = if !connected {
        [140u8, 145, 155, 255]
    } else {
        match anc_mode {
            Some(1) | Some(3) => [accent[0], accent[1], accent[2], 255],
            Some(2) => [60, 210, 175, 255],
            _ => [130, 225, 130, 255],
        }
    };

    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;

            let d_head = ((fx - 13.5).powi(2) + (fy - 12.5).powi(2)).sqrt() - 8.2;
            let d_stem = sd_round_box(fx - 18.0, fy - 21.0, 2.7, 6.2, 2.4);
            let d_ear = d_head.min(d_stem);
            let d_slit = ((fx - 12.0).powi(2) + (fy - 11.8).powi(2)).sqrt() - 3.0;
            let d_dot = ((fx - 25.0).powi(2) + (fy - 25.0).powi(2)).sqrt() - 4.4;

            let idx = (y * size + x) * 4;
            if d_dot < 0.8 {
                let alpha = (0.8 - d_dot).clamp(0.0, 1.0);
                rgba[idx] = dot_color[0];
                rgba[idx + 1] = dot_color[1];
                rgba[idx + 2] = dot_color[2];
                rgba[idx + 3] = (alpha * 255.0) as u8;
            } else if d_ear < 0.8 {
                let alpha = (0.8 - d_ear).clamp(0.0, 1.0);
                if d_slit < 0.0 {
                    rgba[idx] = accent[0];
                    rgba[idx + 1] = accent[1];
                    rgba[idx + 2] = accent[2];
                    rgba[idx + 3] = (alpha * 255.0) as u8;
                } else {
                    rgba[idx] = 244;
                    rgba[idx + 1] = 244;
                    rgba[idx + 2] = 248;
                    rgba[idx + 3] = (alpha * 255.0) as u8;
                }
            }
        }
    }

    Icon::from_rgba(rgba, size as u32, size as u32).expect("valid icon")
}

fn sd_round_box(px: f32, py: f32, bx: f32, by: f32, r: f32) -> f32 {
    let qx = px.abs() - bx + r;
    let qy = py.abs() - by + r;
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
}