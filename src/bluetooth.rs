use crate::models::{resolve_model, ModelProfile};
use crate::protocol::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// ---------------- Win32 FFI (no fragile struct-version drift) ----------------

const AF_BTH: u16 = 32;
const SOCK_STREAM: i32 = 1;
const BTHPROTO_RFCOMM: i32 = 3;
const SOL_SOCKET: i32 = 0xFFFF;
const SO_RCVTIMEO: i32 = 0x1006;
const INVALID_SOCKET: usize = usize::MAX;
const SOCKET_ERROR: i32 = -1;
const WSAETIMEDOUT: i32 = 10060;

#[repr(C)]
#[derive(Clone, Copy)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

// Standard Bluetooth Serial Port Profile UUID: 00001101-0000-1000-8000-00805F9B34FB
const SPP_SERVICE_UUID: Guid = Guid {
    data1: 0x00001101,
    data2: 0x0000,
    data3: 0x1000,
    data4: [0x80, 0x00, 0x00, 0x80, 0x5F, 0x9B, 0x34, 0xFB],
};

#[repr(C, packed(1))]
struct SockAddrBth {
    address_family: u16,
    bt_addr: u64,
    service_class_id: Guid,
    port: u32,
}

#[repr(C)]
struct WsaData {
    w_version: u16,
    w_high_version: u16,
    #[cfg(target_pointer_width = "64")]
    i_max_sockets: u16,
    #[cfg(target_pointer_width = "64")]
    i_max_udp_dg: u16,
    #[cfg(target_pointer_width = "64")]
    lp_vendor_info: *mut u8,
    sz_description: [u8; 257],
    sz_system_status: [u8; 129],
    #[cfg(target_pointer_width = "32")]
    i_max_sockets: u16,
    #[cfg(target_pointer_width = "32")]
    i_max_udp_dg: u16,
    #[cfg(target_pointer_width = "32")]
    lp_vendor_info: *mut u8,
}

#[repr(C)]
struct BluetoothDeviceSearchParams {
    dw_size: u32,
    f_return_authenticated: i32,
    f_return_remembered: i32,
    f_return_unknown: i32,
    f_return_connected: i32,
    f_issue_inquiry: i32,
    c_timeout_multiplier: u8,
    h_radio: isize,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SystemTime {
    w_year: u16,
    w_month: u16,
    w_day_of_week: u16,
    w_day: u16,
    w_hour: u16,
    w_minute: u16,
    w_second: u16,
    w_milliseconds: u16,
}

#[repr(C)]
struct BluetoothDeviceInfo {
    dw_size: u32,
    address: u64,
    ul_class_of_device: u32,
    f_connected: i32,
    f_remembered: i32,
    f_authenticated: i32,
    st_last_seen: SystemTime,
    st_last_used: SystemTime,
    sz_name: [u16; 248],
}

#[link(name = "ws2_32")]
extern "system" {
    fn WSAStartup(w_version_requested: u16, lp_wsa_data: *mut WsaData) -> i32;
    fn socket(af: i32, socket_type: i32, protocol: i32) -> usize;
    fn connect(s: usize, name: *const c_void, namelen: i32) -> i32;
    fn send(s: usize, buf: *const u8, len: i32, flags: i32) -> i32;
    fn recv(s: usize, buf: *mut u8, len: i32, flags: i32) -> i32;
    fn closesocket(s: usize) -> i32;
    fn setsockopt(s: usize, level: i32, optname: i32, optval: *const u8, optlen: i32) -> i32;
    fn WSAGetLastError() -> i32;
}

#[link(name = "Bthprops")]
extern "system" {
    fn BluetoothFindFirstDevice(
        pbtsp: *const BluetoothDeviceSearchParams,
        pbtdi: *mut BluetoothDeviceInfo,
    ) -> isize;
    fn BluetoothFindNextDevice(hfind: isize, pbtdi: *mut BluetoothDeviceInfo) -> i32;
    fn BluetoothFindDeviceClose(hfind: isize) -> i32;
}

pub fn init_winsock() {
    unsafe {
        let mut data: WsaData = std::mem::zeroed();
        let _ = WSAStartup(0x0202, &mut data);
    }
}

// ---------------- Shared App & Device State ----------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedDevice {
    pub name: String,
    pub address: u64,
    pub address_str: String,
    pub connected: bool,
    pub likely_soundpeats: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameLogEntry {
    pub dir: String, // "TX" | "RX"
    pub hex: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceState {
    pub connection_status: String, // "disconnected" | "connecting" | "probing" | "connected" | "demo"
    pub status_message: String,
    pub demo_mode: bool,
    pub bt_name: String,
    pub bt_address: u64,
    pub firmware: Option<String>,
    pub model: ModelProfile,
    pub supported: HashSet<String>,

    pub battery_left: Option<u8>,
    pub battery_right: Option<u8>,
    pub battery_case: Option<u8>,

    pub anc_mode: Option<u8>,
    pub anc_level: Option<u8>,
    pub anc_levels_supported: Vec<u8>,
    pub transparency_level: Option<u8>,
    pub transparency_levels_supported: Vec<u8>,
    pub anc_env_adaptive: Option<bool>,

    pub game_mode: Option<bool>,
    pub multipoint: Option<bool>,
    pub ldac: Option<bool>,
    pub le_audio: Option<bool>,
    pub in_ear: Option<bool>,
    pub touch_disabled: Option<bool>,
    pub dynamic_eq: Option<bool>,
    pub dynamic_bass: Option<bool>,
    pub movie_mode: Option<bool>,
    pub spatial_audio: Option<bool>,
    pub panoramic: Option<u8>,
    pub dolby: Option<bool>,
    pub adaptive_volume: Option<bool>,
    pub light_effect: Option<bool>,
    pub privacy_mode: Option<bool>,

    pub prompt_volume: Option<u8>,
    pub auto_shutdown: Option<u8>,
    pub voice_lang: Option<u8>,
    pub custom_keys: Vec<(u8, u8)>,

    pub active_preset: String,
    pub custom_slider_gains: Vec<f32>,
    pub frame_log: Vec<FrameLogEntry>,
}

impl Default for DeviceState {
    fn default() -> Self {
        let model = resolve_model(Some("S5"), "SOUNDPEATS H3");
        let sliders = vec![0.0; model.custom_slider_freqs.len()];
        Self {
            connection_status: "disconnected".into(),
            status_message: "Select paired earbuds or try Demo Mode".into(),
            demo_mode: false,
            bt_name: String::new(),
            bt_address: 0,
            firmware: None,
            model,
            supported: HashSet::new(),
            battery_left: None,
            battery_right: None,
            battery_case: None,
            anc_mode: None,
            anc_level: None,
            anc_levels_supported: Vec::new(),
            transparency_level: None,
            transparency_levels_supported: Vec::new(),
            anc_env_adaptive: None,
            game_mode: None,
            multipoint: None,
            ldac: None,
            le_audio: None,
            in_ear: None,
            touch_disabled: None,
            dynamic_eq: None,
            dynamic_bass: None,
            movie_mode: None,
            spatial_audio: None,
            panoramic: None,
            dolby: None,
            adaptive_volume: None,
            light_effect: None,
            privacy_mode: None,
            prompt_volume: None,
            auto_shutdown: None,
            voice_lang: None,
            custom_keys: Vec::new(),
            active_preset: "SOUNDPEATS Classic".into(),
            custom_slider_gains: sliders,
            frame_log: Vec::new(),
        }
    }
}

pub fn format_bt_addr(addr: u64) -> String {
    let b = addr.to_be_bytes();
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        b[2], b[3], b[4], b[5], b[6], b[7]
    )
}

pub fn enumerate_paired_devices() -> Vec<PairedDevice> {
    let mut list = Vec::new();
    unsafe {
        let params = BluetoothDeviceSearchParams {
            dw_size: std::mem::size_of::<BluetoothDeviceSearchParams>() as u32,
            f_return_authenticated: 1,
            f_return_remembered: 1,
            f_return_unknown: 0,
            f_return_connected: 1,
            f_issue_inquiry: 0,
            c_timeout_multiplier: 0,
            h_radio: 0,
        };
        let mut info: BluetoothDeviceInfo = std::mem::zeroed();
        info.dw_size = std::mem::size_of::<BluetoothDeviceInfo>() as u32;

        let handle = BluetoothFindFirstDevice(&params, &mut info);
        if handle != 0 {
            loop {
                let end = info.sz_name.iter().position(|&c| c == 0).unwrap_or(info.sz_name.len());
                let name = String::from_utf16_lossy(&info.sz_name[..end]);
                let lower = name.to_lowercase();
                let likely = lower.contains("soundpeats")
                    || lower.contains("peats")
                    || lower.contains("truefree")
                    || lower.contains("capsule")
                    || lower.contains("air");
                if !name.is_empty() {
                    list.push(PairedDevice {
                        name,
                        address: info.address,
                        address_str: format_bt_addr(info.address),
                        connected: info.f_connected != 0,
                        likely_soundpeats: likely,
                    });
                }
                if BluetoothFindNextDevice(handle, &mut info) == 0 {
                    break;
                }
            }
            BluetoothFindDeviceClose(handle);
        }
    }

    list.sort_by(|a, b| {
        b.likely_soundpeats
            .cmp(&a.likely_soundpeats)
            .then(b.connected.cmp(&a.connected))
            .then(a.name.cmp(&b.name))
    });
    list
}

// ---------------- Transport Controller ----------------

pub enum TransportCmd {
    SendFrame(GaiaFrame, String),
    Disconnect,
}

#[derive(Clone)]
pub struct BluetoothManager {
    pub state: Arc<Mutex<DeviceState>>,
    tx: Arc<Mutex<Option<Sender<TransportCmd>>>>,
    stop_flag: Arc<Mutex<Option<Arc<AtomicBool>>>>,
    on_update: Arc<dyn Fn() + Send + Sync>,
}

impl BluetoothManager {
    pub fn new<F: Fn() + Send + Sync + 'static>(on_update: F) -> Self {
        init_winsock();
        Self {
            state: Arc::new(Mutex::new(DeviceState::default())),
            tx: Arc::new(Mutex::new(None)),
            stop_flag: Arc::new(Mutex::new(None)),
            on_update: Arc::new(on_update),
        }
    }

    pub fn disconnect(&self) {
        if let Some(flag) = self.stop_flag.lock().unwrap().take() {
            flag.store(true, Ordering::SeqCst);
        }
        if let Some(tx) = self.tx.lock().unwrap().take() {
            let _ = tx.send(TransportCmd::Disconnect);
        }
        let mut st = self.state.lock().unwrap();
        st.connection_status = "disconnected".into();
        st.demo_mode = false;
        st.status_message = "Disconnected".into();
        drop(st);
        (self.on_update)();
    }

    pub fn start_demo(&self) {
        self.disconnect();
        {
            let mut st = self.state.lock().unwrap();
            let model = resolve_model(Some("S5_20250605_V0.4.9"), "SOUNDPEATS H3");
            let sliders = vec![0.0; model.custom_slider_freqs.len()];
            *st = DeviceState {
                connection_status: "demo".into(),
                status_message: "Demo Mode · Virtual H3 (S5)".into(),
                demo_mode: true,
                bt_name: "SOUNDPEATS H3 (Demo)".into(),
                bt_address: 0x9880BB123456,
                firmware: Some("S5_20250605_V0.4.9".into()),
                model,
                supported: [
                    "battery",
                    "anc_mode",
                    "anc_level",
                    "transparency_level",
                    "game_mode",
                    "multipoint",
                    "ldac",
                    "le_audio",
                    "disable_touch",
                    "custom_keys",
                    "prompt_volume",
                    "voice_lang",
                    "find",
                ]
                .into_iter()
                .map(String::from)
                .collect(),
                battery_left: Some(96),
                battery_right: Some(92),
                battery_case: Some(74),
                anc_mode: Some(1),
                anc_level: Some(0x11),
                anc_levels_supported: vec![0x11, 0x12, 0x13, 0x14],
                transparency_level: Some(2),
                transparency_levels_supported: vec![1, 2],
                game_mode: Some(false),
                multipoint: Some(false),
                ldac: Some(true),
                le_audio: Some(false),
                touch_disabled: Some(false),
                prompt_volume: Some(18),
                voice_lang: Some(0),
                custom_keys: vec![
                    (1, 2),
                    (2, 1),
                    (3, 3),
                    (4, 3),
                    (5, 5),
                    (6, 6),
                    (7, 7),
                    (8, 8),
                ],
                active_preset: "SOUNDPEATS Classic".into(),
                custom_slider_gains: sliders,
                frame_log: vec![
                    FrameLogEntry {
                        dir: "TX".into(),
                        hex: GaiaFrame::ctrl(GET_FIRMWARE, &[]).to_hex(),
                        label: "GET_FIRMWARE".into(),
                    },
                    FrameLogEntry {
                        dir: "RX".into(),
                        hex: "FF 03 00 13 00 0A 83 09 00 53 35 5F 32 30 32 35 30 36 30 35 5F 56 30 2E 34 2E 39".into(),
                        label: "S5_20250605_V0.4.9".into(),
                    },
                ],
                ..Default::default()
            };
        }
        (self.on_update)();
    }

    pub fn connect_device(&self, addr: u64, name: String) {
        self.disconnect();
        {
            let mut st = self.state.lock().unwrap();
            *st = DeviceState::default();
            st.connection_status = "connecting".into();
            st.bt_name = name.clone();
            st.bt_address = addr;
            st.status_message = format!("Opening RFCOMM SPP to {}...", name);
            st.model = resolve_model(None, &name);
            st.custom_slider_gains = vec![0.0; st.model.custom_slider_freqs.len()];
        }
        (self.on_update)();

        let (cmd_tx, cmd_rx): (Sender<TransportCmd>, Receiver<TransportCmd>) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        *self.tx.lock().unwrap() = Some(cmd_tx.clone());
        *self.stop_flag.lock().unwrap() = Some(stop.clone());

        let state_arc = self.state.clone();
        let on_update = self.on_update.clone();

        thread::spawn(move || {
            let sock = unsafe {
                let s = socket(AF_BTH as i32, SOCK_STREAM, BTHPROTO_RFCOMM);
                if s == INVALID_SOCKET {
                    update_error(&state_arc, &on_update, "Failed to create AF_BTH socket");
                    return;
                }
                let timeout_ms: u32 = 250;
                setsockopt(
                    s,
                    SOL_SOCKET,
                    SO_RCVTIMEO,
                    &timeout_ms as *const u32 as *const u8,
                    4,
                );
                let sa = SockAddrBth {
                    address_family: AF_BTH,
                    bt_addr: addr,
                    service_class_id: SPP_SERVICE_UUID,
                    port: 0, // SDP auto-resolve RFCOMM channel
                };
                let res = connect(
                    s,
                    &sa as *const SockAddrBth as *const c_void,
                    std::mem::size_of::<SockAddrBth>() as i32,
                );
                if res == SOCKET_ERROR {
                    let err = WSAGetLastError();
                    closesocket(s);
                    update_error(
                        &state_arc,
                        &on_update,
                        &format!(
                            "SPP connect failed (WSA {}). Ensure earbuds are awake & PeatsAudio is closed.",
                            err
                        ),
                    );
                    return;
                }
                s
            };

            {
                let mut st = state_arc.lock().unwrap();
                st.connection_status = "probing".into();
                st.status_message = "Probing supported features...".into();
            }
            (on_update)();

            // Spawn reader thread
            let reader_stop = stop.clone();
            let reader_state = state_arc.clone();
            let reader_update = on_update.clone();
            let reader_handle = thread::spawn(move || {
                let mut reassembler = FrameReader::default();
                let mut buf = [0u8; 1024];
                while !reader_stop.load(Ordering::SeqCst) {
                    let n = unsafe { recv(sock, buf.as_mut_ptr(), buf.len() as i32, 0) };
                    if n > 0 {
                        let frames = reassembler.push(&buf[..n as usize]);
                        if !frames.is_empty() {
                            let mut st = reader_state.lock().unwrap();
                            for f in frames {
                                apply_rx_frame(&mut st, &f);
                            }
                            drop(st);
                            (reader_update)();
                        }
                    } else if n == 0 {
                        // Peer closed channel (e.g., LDAC/Multipoint restart on H3)
                        if !reader_stop.load(Ordering::SeqCst) {
                            let mut st = reader_state.lock().unwrap();
                            st.connection_status = "disconnected".into();
                            st.status_message = "Channel closed by earbuds (reconnect if toggling LDAC/Multipoint)".into();
                            drop(st);
                            (reader_update)();
                        }
                        break;
                    } else {
                        let err = unsafe { WSAGetLastError() };
                        if err != WSAETIMEDOUT {
                            if !reader_stop.load(Ordering::SeqCst) {
                                let mut st = reader_state.lock().unwrap();
                                st.connection_status = "disconnected".into();
                                st.status_message = format!("RFCOMM disconnected (WSA {})", err);
                                drop(st);
                                (reader_update)();
                            }
                            break;
                        }
                    }
                }
            });

            // Probe sequence (50ms write pacing, never waiting on reply)
            for &cmd in PROBE_SEQUENCE {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                let frame = GaiaFrame::ctrl(cmd, &[]);
                if !send_raw(sock, &frame, &state_arc, &on_update, format!("PROBE {:04X}", cmd)) {
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }

            if !stop.load(Ordering::SeqCst) {
                let mut st = state_arc.lock().unwrap();
                if st.connection_status == "probing" {
                    st.connection_status = "connected".into();
                    st.status_message = format!(
                        "{} · {}",
                        st.model.display_name,
                        st.firmware.as_deref().unwrap_or("Connected")
                    );
                }
                drop(st);
                (on_update)();
            }

            // Paced writer loop
            while let Ok(cmd) = cmd_rx.recv() {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                match cmd {
                    TransportCmd::Disconnect => break,
                    TransportCmd::SendFrame(frame, label) => {
                        if !send_raw(sock, &frame, &state_arc, &on_update, label) {
                            break;
                        }
                        thread::sleep(Duration::from_millis(50));
                    }
                }
            }

            stop.store(true, Ordering::SeqCst);
            unsafe {
                closesocket(sock);
            }
            let _ = reader_handle.join();
        });
    }

    pub fn send_ctrl(&self, cmd: u16, payload: &[u8], label: &str) {
        let demo = self.state.lock().unwrap().demo_mode;
        let frame = GaiaFrame::ctrl(cmd, payload);
        if demo {
            self.simulate_ctrl(cmd, payload, &frame, label);
            return;
        }
        if let Some(tx) = self.tx.lock().unwrap().as_ref() {
            let _ = tx.send(TransportCmd::SendFrame(frame, label.to_string()));
        }
    }

    pub fn send_eq(&self, eq_cmd: EqCommandKind, master_db: f32, bands: &[EqBand]) {
        let frames = encode_eq_frames(eq_cmd, master_db, bands);
        let demo = self.state.lock().unwrap().demo_mode;
        if demo {
            let mut st = self.state.lock().unwrap();
            for (i, f) in frames.iter().enumerate() {
                push_log(&mut st, "TX", &f.to_hex(), &format!("EQ band {}", i + 1));
            }
            push_log(&mut st, "RX", "FF 03 00 01 00 1D 0F 81 01", "EQ ACK");
            drop(st);
            (self.on_update)();
            return;
        }
        if let Some(tx) = self.tx.lock().unwrap().as_ref() {
            for (i, f) in frames.into_iter().enumerate() {
                let _ = tx.send(TransportCmd::SendFrame(f, format!("EQ band {}", i + 1)));
            }
        }
    }

    fn simulate_ctrl(&self, cmd: u16, payload: &[u8], frame: &GaiaFrame, label: &str) {
        let mut st = self.state.lock().unwrap();
        push_log(&mut st, "TX", &frame.to_hex(), label);
        let val = payload.first().copied().unwrap_or(0);
        match cmd {
            SET_ANC_MODE => {
                st.anc_mode = Some(val);
                let reply = GaiaFrame::ctrl(0x8310, &[0x00, val]);
                push_log(&mut st, "RX", &reply.to_hex(), "ANC_MODE update");
            }
            SET_ANC_LEVEL => {
                st.anc_level = Some(val);
                let reply = GaiaFrame::ctrl(0x8324, &[0x00, val, 0x01]);
                push_log(&mut st, "RX", &reply.to_hex(), "ANC_LEVEL update");
            }
            SET_TRANSPARENCY_LEVEL => {
                st.transparency_level = Some(val);
                let reply = GaiaFrame::ctrl(0x834C, &[0x00, val, 0x01]);
                push_log(&mut st, "RX", &reply.to_hex(), "TRANSPARENCY_LEVEL update");
            }
            SET_GAME_MODE => st.game_mode = Some(val != 0),
            SET_MULTIPOINT => {
                st.multipoint = Some(val != 0);
                if st.model.ldac_or_multipoint && val != 0 {
                    st.ldac = Some(false);
                }
            }
            SET_LDAC => {
                st.ldac = Some(val != 0);
                if st.model.ldac_or_multipoint && val != 0 {
                    st.multipoint = Some(false);
                }
            }
            SET_LE_AUDIO => st.le_audio = Some(val != 0),
            SET_DISABLE_TOUCH => {
                let logical = if st.model.touch_inverted { val == 0 } else { val != 0 };
                st.touch_disabled = Some(logical);
            }
            SET_PROMPT_VOLUME => st.prompt_volume = Some(val),
            SET_VOICE_LANG => st.voice_lang = Some(val),
            SET_CUSTOM_KEYS if payload.len() >= 2 => {
                let g = payload[0];
                let a = payload[1];
                if let Some(item) = st.custom_keys.iter_mut().find(|x| x.0 == g) {
                    item.1 = a;
                } else {
                    st.custom_keys.push((g, a));
                }
            }
            CMD_RESET_CUSTOM_KEYS => {
                st.custom_keys = vec![
                    (1, 2), (2, 1), (3, 3), (4, 3), (5, 5), (6, 6), (7, 7), (8, 8),
                ];
            }
            _ => {}
        }
        drop(st);
        (self.on_update)();
    }
}

fn send_raw(
    sock: usize,
    frame: &GaiaFrame,
    state_arc: &Arc<Mutex<DeviceState>>,
    on_update: &Arc<dyn Fn() + Send + Sync>,
    label: String,
) -> bool {
    let bytes = frame.encode();
    let res = unsafe { send(sock, bytes.as_ptr(), bytes.len() as i32, 0) };
    if res == SOCKET_ERROR {
        return false;
    }
    let mut st = state_arc.lock().unwrap();
    push_log(&mut st, "TX", &frame.to_hex(), &label);
    drop(st);
    (on_update)();
    true
}

fn update_error(
    state_arc: &Arc<Mutex<DeviceState>>,
    on_update: &Arc<dyn Fn() + Send + Sync>,
    msg: &str,
) {
    let mut st = state_arc.lock().unwrap();
    st.connection_status = "disconnected".into();
    st.status_message = msg.into();
    drop(st);
    (on_update)();
}

fn push_log(st: &mut DeviceState, dir: &str, hex: &str, label: &str) {
    st.frame_log.push(FrameLogEntry {
        dir: dir.into(),
        hex: hex.into(),
        label: label.into(),
    });
    if st.frame_log.len() > 80 {
        let excess = st.frame_log.len() - 80;
        st.frame_log.drain(0..excess);
    }
}

fn apply_rx_frame(st: &mut DeviceState, frame: &GaiaFrame) {
    push_log(
        st,
        "RX",
        &frame.to_hex(),
        &format!("V:{:04X} C:{:04X}", frame.vendor, frame.command),
    );

    if frame.vendor == VENDOR_EQ {
        return;
    }
    if frame.vendor != VENDOR_CTRL {
        return;
    }

    match frame.command {
        0x8309 => {
            // Firmware version: status 0x00 + ASCII
            let raw = if frame.payload.first() == Some(&0x00) {
                &frame.payload[1..]
            } else {
                &frame.payload
            };
            let ver = String::from_utf8_lossy(raw).trim_matches('\0').trim().to_string();
            if !ver.is_empty() {
                st.firmware = Some(ver.clone());
                let prev_name = st.bt_name.clone();
                st.model = resolve_model(Some(&ver), &prev_name);
                if st.custom_slider_gains.len() != st.model.custom_slider_freqs.len() {
                    st.custom_slider_gains = vec![0.0; st.model.custom_slider_freqs.len()];
                }
                if let Some(first_preset) = st.model.presets.first() {
                    st.active_preset = first_preset.name.clone();
                }
            }
        }
        0x8306 => {
            if let Some(v) = parse_status_u8(&frame.payload) {
                st.supported.insert("battery".into());
                st.battery_left = if v == 0xFF { None } else { Some(v.min(100)) };
            }
        }
        0x8307 => {
            if let Some(v) = parse_status_u8(&frame.payload) {
                st.supported.insert("battery".into());
                st.battery_right = if v == 0xFF { None } else { Some(v.min(100)) };
            }
        }
        0x8323 => {
            if let Some(v) = parse_status_u8(&frame.payload) {
                st.supported.insert("battery".into());
                st.battery_case = if v == 0xFF { None } else { Some(v.min(100)) };
            }
        }
        0x8310 => {
            if let Some(v) = parse_status_u8(&frame.payload) {
                st.supported.insert("anc_mode".into());
                st.anc_mode = Some(v);
            }
        }
        0x8324 => {
            let (levels, active) = parse_level_pairs(&frame.payload);
            if !levels.is_empty() {
                st.supported.insert("anc_level".into());
                st.anc_levels_supported = levels;
                if active.is_some() {
                    st.anc_level = active;
                }
            }
        }
        0x834C => {
            let (levels, active) = parse_level_pairs(&frame.payload);
            if !levels.is_empty() {
                st.supported.insert("transparency_level".into());
                st.transparency_levels_supported = levels;
                if active.is_some() {
                    st.transparency_level = active;
                }
            } else if let Some(v) = parse_status_u8(&frame.payload) {
                st.supported.insert("transparency_level".into());
                st.transparency_level = Some(v);
            }
        }
        0x8364 => {
            if let Some(v) = parse_status_u8(&frame.payload) {
                st.supported.insert("anc_env_adaptive".into());
                st.anc_env_adaptive = Some(v != 0);
            }
        }
        0x830E => {
            if let Some(v) = parse_status_u8(&frame.payload) {
                st.supported.insert("game_mode".into());
                st.game_mode = Some(v != 0);
            }
        }
        0x8314 => {
            if let Some(v) = parse_status_u8(&frame.payload) {
                st.supported.insert("multipoint".into());
                st.multipoint = Some(v != 0);
            }
        }
        0x8338 => {
            if let Some(v) = parse_status_u8(&frame.payload) {
                st.supported.insert("ldac".into());
                st.ldac = Some(v != 0);
            }
        }
        0x8335 => {
            if let Some(v) = parse_status_u8(&frame.payload) {
                st.supported.insert("le_audio".into());
                st.le_audio = Some(v != 0);
            }
        }
        0x830C => {
            if let Some(v) = parse_status_u8(&frame.payload) {
                st.supported.insert("in_ear".into());
                st.in_ear = Some(v != 0);
            }
        }
        0x8312 => {
            if let Some(v) = parse_status_u8(&frame.payload) {
                st.supported.insert("disable_touch".into());
                let raw_on = v != 0;
                st.touch_disabled = Some(if st.model.touch_inverted { !raw_on } else { raw_on });
            }
        }
        0x83AA => {
            let is_qc = st.model.family == crate::models::ChipFamily::Qualcomm;
            let keys = parse_custom_keys(&frame.payload, is_qc);
            if !keys.is_empty() {
                st.supported.insert("custom_keys".into());
                st.custom_keys = keys;
            }
        }
        0x8319 => {
            if let Some(v) = parse_status_u8(&frame.payload) {
                st.supported.insert("prompt_volume".into());
                st.prompt_volume = Some(v);
            }
        }
        0x836E => {
            if let Some(v) = parse_status_u8(&frame.payload) {
                st.supported.insert("auto_shutdown".into());
                st.auto_shutdown = Some(v);
            }
        }
        0x8321 => {
            if let Some(v) = parse_status_u8(&frame.payload) {
                st.supported.insert("voice_lang".into());
                st.voice_lang = Some(v);
            }
        }
        0x8331 => {
            if let Some(v) = parse_status_u8(&frame.payload) {
                st.supported.insert("dynamic_eq".into());
                st.dynamic_eq = Some(v != 0);
            }
        }
        0x833C => {
            if let Some(v) = parse_status_u8(&frame.payload) {
                st.supported.insert("dynamic_bass".into());
                st.dynamic_bass = Some(v != 0);
            }
        }
        0x8327 => {
            if let Some(v) = parse_status_u8(&frame.payload) {
                st.supported.insert("movie_mode".into());
                st.movie_mode = Some(v != 0);
            }
        }
        0x8357 => {
            if let Some(v) = parse_status_u8(&frame.payload) {
                st.supported.insert("spatial_audio".into());
                st.spatial_audio = Some(v != 0);
            }
        }
        0x8329 => {
            if let Some(v) = parse_status_u8(&frame.payload) {
                st.supported.insert("panoramic".into());
                st.panoramic = Some(v);
            }
        }
        0x8348 => {
            if let Some(v) = parse_status_u8(&frame.payload) {
                st.supported.insert("dolby".into());
                st.dolby = Some(v != 0);
            }
        }
        0x8346 => {
            if let Some(v) = parse_status_u8(&frame.payload) {
                st.supported.insert("adaptive_volume".into());
                st.adaptive_volume = Some(v != 0);
            }
        }
        0x830A => {
            if let Some(v) = parse_status_u8(&frame.payload) {
                st.supported.insert("light_effect".into());
                st.light_effect = Some(v != 0);
            }
        }
        0x836B => {
            if let Some(v) = parse_status_u8(&frame.payload) {
                st.supported.insert("privacy_mode".into());
                st.privacy_mode = Some(v != 0);
            }
        }
        0x834A | 0x834B => {
            st.supported.insert("find".into());
        }
        0x8317 => {
            // Unasked "connect classic" frame seen on H3; logged above, never trigger 03 17.
        }
        _ => {}
    }
}