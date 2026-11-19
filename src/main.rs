#![windows_subsystem = "windows"]

mod bluetooth;
mod models;
mod protocol;
mod theme;

use bluetooth::{enumerate_paired_devices, BluetoothManager};
use models::compose_custom_eq;
use protocol::*;
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tao::{
    dpi::{LogicalSize, PhysicalPosition},
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::windows::WindowExtWindows,
    window::WindowBuilder,
};
use tray_icon::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent,
};
use wry::WebViewBuilder;

enum UserEvent {
    Tray(TrayIconEvent),
    Menu(MenuEvent),
    BtUpdated,
    Ipc(String),
}

#[derive(Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum IpcAction {
    Ready,
    ScanDevices,
    Connect { address: String, name: String },
    Disconnect,
    StartDemo,
    SetAncMode { mode: u8 },
    SetAncLevel { level: u8 },
    SetTransparencyLevel { level: u8 },
    SetBool { feature: String, value: bool },
    SetU8 { feature: String, value: u8 },
    SetCustomKey { gesture: u8, action: u8 },
    ResetCustomKeys,
    ApplyPreset { name: String },
    ApplyCustomEq { gains: Vec<f32> },
    FindEarbud { side: String, on: bool },
    SetPinned { pinned: bool },
    SetTheme { palette_idx: i32 },
    Hide,
    Quit,
}

fn build_embedded_html() -> String {
    let html = include_str!("../ui/index.html");
    let css = include_str!("../ui/styles.css");
    let js = include_str!("../ui/app.js");
    html.replace("/*__APP_CSS__*/", css)
        .replace("//__APP_JS__", js)
}

fn main() {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let tray_proxy = proxy.clone();
    TrayIconEvent::set_event_handler(Some(move |event| {
        let _ = tray_proxy.send_event(UserEvent::Tray(event));
    }));

    let menu_proxy = proxy.clone();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = menu_proxy.send_event(UserEvent::Menu(event));
    }));

    let sys_theme = theme::read_system_theme();
    let config = theme::load_config();
    let palette_state = Arc::new(AtomicI32::new(config.palette_idx));

    let menu = Menu::new();
    let m_show = MenuItem::with_id("show", "Open PeatsDesktop", true, None);
    let m_anc_off = MenuItem::with_id("anc_0", "Noise Control: Off", true, None);
    let m_anc_on = MenuItem::with_id("anc_1", "Noise Control: ANC On", true, None);
    let m_anc_tr = MenuItem::with_id("anc_2", "Noise Control: Transparency", true, None);
    let m_game = MenuItem::with_id("game_toggle", "Toggle Game Mode", true, None);
    let m_demo = MenuItem::with_id("demo", "Launch Demo Mode (H3)", true, None);
    let m_startup = CheckMenuItem::with_id(
        "startup",
        "Start with Windows",
        true,
        theme::is_startup_enabled(),
        None,
    );
    let m_quit = MenuItem::with_id("quit", "Quit PeatsDesktop", true, None);
    let _ = menu.append_items(&[
        &m_show,
        &PredefinedMenuItem::separator(),
        &m_anc_off,
        &m_anc_on,
        &m_anc_tr,
        &m_game,
        &PredefinedMenuItem::separator(),
        &m_demo,
        &m_startup,
        &PredefinedMenuItem::separator(),
        &m_quit,
    ]);

    let initial_icon = theme::build_tray_icon(sys_theme.accent_rgb, false, None);
    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .with_tooltip("PeatsDesktop · Not connected")
        .with_icon(initial_icon)
        .build()
        .expect("tray icon");

    let window = WindowBuilder::new()
        .with_title("PeatsDesktop")
        .with_inner_size(LogicalSize::new(412.0, 650.0))
        .with_decorations(false)
        .with_resizable(false)
        .with_visible(false)
        .with_always_on_top(true)
        .build(&event_loop)
        .expect("window");

    let _ = window.set_skip_taskbar(true);
    theme::apply_dwm_flyout_style(window.hwnd(), sys_theme.dark_mode);

    let ipc_proxy = proxy.clone();
    let bg = if sys_theme.dark_mode {
        (18, 19, 24, 255)
    } else {
        (250, 248, 255, 255)
    };

    let webview = WebViewBuilder::new()
        .with_background_color(bg)
        .with_html(build_embedded_html())
        .with_ipc_handler(move |req| {
            let _ = ipc_proxy.send_event(UserEvent::Ipc(req.body().clone()));
        })
        .build(&window)
        .expect("webview");

    let bt_proxy = proxy.clone();
    let bt = BluetoothManager::new(move || {
        let _ = bt_proxy.send_event(UserEvent::BtUpdated);
    });

    let pinned = Arc::new(AtomicBool::new(false));
    let mut last_tray_toggle = Instant::now();
    let mut last_tray_status: (bool, Option<u8>) = (false, None);
    let palette_for_push = palette_state.clone();

    let mut push_state = move |webview: &wry::WebView, bt: &BluetoothManager, tray: &tray_icon::TrayIcon| {
        let st = bt.state.lock().unwrap().clone();
        let theme_now = theme::read_system_theme();
        let connected = matches!(st.connection_status.as_str(), "connected" | "demo");

        if last_tray_status != (connected, st.anc_mode) {
            let icon = theme::build_tray_icon(theme_now.accent_rgb, connected, st.anc_mode);
            let _ = tray.set_icon(Some(icon));
            last_tray_status = (connected, st.anc_mode);
        }

        let tip = if connected {
            let l = st.battery_left.map(|v| format!("L {}%", v)).unwrap_or_default();
            let r = st.battery_right.map(|v| format!("R {}%", v)).unwrap_or_default();
            let c = st.battery_case.map(|v| format!("Case {}%", v)).unwrap_or_default();
            let anc = match st.anc_mode {
                Some(0) => "ANC Off",
                Some(1) => "ANC On",
                Some(2) => "Transparency",
                Some(3) => "Adaptive",
                _ => "Connected",
            };
            format!("{} · {} {} {} · {}", st.model.display_name, l, r, c, anc)
        } else {
            format!("PeatsDesktop · {}", st.status_message)
        };
        let _ = tray.set_tooltip(Some(&tip));

        let payload = serde_json::json!({
            "state": st,
            "theme": theme_now,
            "palette_idx": palette_for_push.load(Ordering::Relaxed),
        });
        let js = format!("window.__onState && window.__onState({});", payload);
        let _ = webview.evaluate_script(&js);
    };

    let push_devices = |webview: &wry::WebView| {
        let devs = enumerate_paired_devices();
        let json = serde_json::to_string(&devs).unwrap_or_else(|_| "[]".into());
        let js = format!("window.__onDevices && window.__onDevices({});", json);
        let _ = webview.evaluate_script(&js);
    };

    let show_flyout = |window: &tao::window::Window, click_pos: Option<PhysicalPosition<f64>>| {
        let work = theme::get_work_area();
        let size = window.outer_size();
        let w = size.width as i32;
        let h = size.height as i32;
        let margin = 12;

        let x = if let Some(pos) = click_pos {
            (pos.x as i32 - w / 2).clamp(work.left + margin, (work.right - w - margin).max(work.left))
        } else {
            (work.right - w - margin).max(work.left)
        };
        let y = (work.bottom - h - margin).max(work.top + margin);

        window.set_outer_position(PhysicalPosition::new(x, y));
        window.set_visible(true);
        window.set_focus();
    };

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(UserEvent::Tray(ev)) => {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    position,
                    ..
                } = ev
                {
                    if window.is_visible() {
                        window.set_visible(false);
                    } else {
                        last_tray_toggle = Instant::now();
                        push_devices(&webview);
                        push_state(&webview, &bt, &tray_icon);
                        show_flyout(&window, Some(position));
                    }
                }
            }
            Event::UserEvent(UserEvent::Menu(ev)) => match ev.id().0.as_str() {
                "show" => {
                    last_tray_toggle = Instant::now();
                    push_devices(&webview);
                    push_state(&webview, &bt, &tray_icon);
                    show_flyout(&window, None);
                }
                "anc_0" => {
                    bt.state.lock().unwrap().anc_mode = Some(0);
                    push_state(&webview, &bt, &tray_icon);
                    bt.send_ctrl(SET_ANC_MODE, &[0], "SET_ANC_MODE 0");
                }
                "anc_1" => {
                    bt.state.lock().unwrap().anc_mode = Some(1);
                    push_state(&webview, &bt, &tray_icon);
                    bt.send_ctrl(SET_ANC_MODE, &[1], "SET_ANC_MODE 1");
                }
                "anc_2" => {
                    bt.state.lock().unwrap().anc_mode = Some(2);
                    push_state(&webview, &bt, &tray_icon);
                    bt.send_ctrl(SET_ANC_MODE, &[2], "SET_ANC_MODE 2");
                }
                "game_toggle" => {
                    let next = !bt.state.lock().unwrap().game_mode.unwrap_or(false);
                    bt.send_ctrl(SET_GAME_MODE, &[u8::from(next)], "SET_GAME_MODE");
                }
                "demo" => {
                    bt.start_demo();
                    show_flyout(&window, None);
                }
                "startup" => {
                    let want = !theme::is_startup_enabled();
                    theme::set_startup_enabled(want);
                    m_startup.set_checked(theme::is_startup_enabled());
                }
                "quit" => {
                    bt.disconnect();
                    *control_flow = ControlFlow::Exit;
                }
                _ => {}
            },
            Event::UserEvent(UserEvent::BtUpdated) => {
                push_state(&webview, &bt, &tray_icon);
            }
            Event::UserEvent(UserEvent::Ipc(raw)) => {
                if let Ok(action) = serde_json::from_str::<IpcAction>(&raw) {
                    match action {
                        IpcAction::Ready => {
                            push_devices(&webview);
                            push_state(&webview, &bt, &tray_icon);
                        }
                        IpcAction::ScanDevices => {
                            push_devices(&webview);
                        }
                        IpcAction::Connect { address, name } => {
                            if let Ok(addr) = address.parse::<u64>() {
                                bt.connect_device(addr, name);
                            }
                        }
                        IpcAction::Disconnect => bt.disconnect(),
                        IpcAction::StartDemo => bt.start_demo(),
                        IpcAction::SetAncMode { mode } => {
                            bt.state.lock().unwrap().anc_mode = Some(mode);
                            push_state(&webview, &bt, &tray_icon);
                            bt.send_ctrl(SET_ANC_MODE, &[mode], &format!("SET_ANC_MODE {}", mode));
                        }
                        IpcAction::SetAncLevel { level } => {
                            bt.state.lock().unwrap().anc_level = Some(level);
                            push_state(&webview, &bt, &tray_icon);
                            bt.send_ctrl(
                                SET_ANC_LEVEL,
                                &[level],
                                &format!("SET_ANC_LEVEL {:02X}", level),
                            );
                        }
                        IpcAction::SetTransparencyLevel { level } => {
                            bt.state.lock().unwrap().transparency_level = Some(level);
                            push_state(&webview, &bt, &tray_icon);
                            bt.send_ctrl(
                                SET_TRANSPARENCY_LEVEL,
                                &[level],
                                &format!("SET_TRANSPARENCY_LEVEL {}", level),
                            );
                        }
                        IpcAction::SetBool { feature, value } => {
                            let inverted = bt.state.lock().unwrap().model.touch_inverted;
                            let wire_val = if feature == "disable_touch" && inverted {
                                u8::from(!value)
                            } else {
                                u8::from(value)
                            };
                            let cmd = match feature.as_str() {
                                "anc_env_adaptive" => Some(SET_ANC_ENV_ADAPTIVE),
                                "game_mode" => Some(SET_GAME_MODE),
                                "multipoint" => Some(SET_MULTIPOINT),
                                "ldac" => Some(SET_LDAC),
                                "le_audio" => Some(SET_LE_AUDIO),
                                "in_ear" => Some(SET_IN_EAR),
                                "disable_touch" => Some(SET_DISABLE_TOUCH),
                                "dynamic_eq" => Some(SET_DYNAMIC_EQ),
                                "dynamic_bass" => Some(SET_DYNAMIC_BASS),
                                "movie_mode" => Some(SET_MOVIE_MODE),
                                "spatial_audio" => Some(SET_SPATIAL_AUDIO),
                                "dolby" => Some(SET_DOLBY),
                                "adaptive_volume" => Some(SET_ADAPTIVE_VOLUME),
                                "light_effect" => Some(SET_LIGHT_EFFECT),
                                "privacy_mode" => Some(SET_PRIVACY_MODE),
                                _ => None,
                            };
                            if let Some(c) = cmd {
                                bt.send_ctrl(c, &[wire_val], &format!("SET {} = {}", feature, wire_val));
                            }
                        }
                        IpcAction::SetU8 { feature, value } => {
                            let cmd = match feature.as_str() {
                                "prompt_volume" => Some(SET_PROMPT_VOLUME),
                                "auto_shutdown" => Some(SET_AUTO_SHUTDOWN),
                                "voice_lang" => Some(SET_VOICE_LANG),
                                "panoramic" => Some(SET_PANORAMIC),
                                _ => None,
                            };
                            if let Some(c) = cmd {
                                bt.send_ctrl(c, &[value], &format!("SET {} = {}", feature, value));
                            }
                        }
                        IpcAction::SetCustomKey { gesture, action } => {
                            bt.send_ctrl(
                                SET_CUSTOM_KEYS,
                                &[gesture, action],
                                &format!("SET_KEY g={} a={}", gesture, action),
                            );
                        }
                        IpcAction::ResetCustomKeys => {
                            bt.send_ctrl(CMD_RESET_CUSTOM_KEYS, &[], "RESET_CUSTOM_KEYS");
                        }
                        IpcAction::ApplyPreset { name } => {
                            let (cmd_kind, master, bands_opt) = {
                                let mut st = bt.state.lock().unwrap();
                                st.active_preset = name.clone();
                                let found = st
                                    .model
                                    .presets
                                    .iter()
                                    .find(|p| p.name == name)
                                    .map(|p| p.bands.clone());
                                (st.model.eq_command, st.model.master_gain_db, found)
                            };
                            if let Some(bands) = bands_opt {
                                bt.send_eq(cmd_kind, master, &bands);
                            }
                            push_state(&webview, &bt, &tray_icon);
                        }
                        IpcAction::ApplyCustomEq { gains } => {
                            let (cmd_kind, master, bands) = {
                                let mut st = bt.state.lock().unwrap();
                                st.active_preset = "Custom".into();
                                st.custom_slider_gains = gains.clone();
                                let bands = compose_custom_eq(&st.model, &gains);
                                (st.model.eq_command, st.model.master_gain_db, bands)
                            };
                            bt.send_eq(cmd_kind, master, &bands);
                            push_state(&webview, &bt, &tray_icon);
                        }
                        IpcAction::FindEarbud { side, on } => {
                            let cmd = if side == "left" {
                                SET_FIND_LEFT
                            } else {
                                SET_FIND_RIGHT
                            };
                            bt.send_ctrl(cmd, &[u8::from(on)], &format!("FIND {} {}", side, on));
                        }
                        IpcAction::SetPinned { pinned: p } => {
                            pinned.store(p, Ordering::SeqCst);
                        }
                        IpcAction::SetTheme { palette_idx } => {
                            palette_state.store(palette_idx, Ordering::Relaxed);
                            theme::save_config(palette_idx);
                        }
                        IpcAction::Hide => {
                            window.set_visible(false);
                        }
                        IpcAction::Quit => {
                            bt.disconnect();
                            *control_flow = ControlFlow::Exit;
                        }
                    }
                }
            }
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::Focused(false) => {
                    if !pinned.load(Ordering::SeqCst)
                        && last_tray_toggle.elapsed().as_millis() > 280
                    {
                        window.set_visible(false);
                    }
                }
                WindowEvent::CloseRequested => {
                    window.set_visible(false);
                }
                _ => {}
            },
            _ => {}
        }
    });
}