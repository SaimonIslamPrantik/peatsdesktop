use serde::{Deserialize, Serialize};

pub const SOF: u8 = 0xFF;
pub const TX_VERSION: u8 = 0x04;
pub const FLAGS: u8 = 0x00;

pub const VENDOR_CTRL: u16 = 0x000A;
pub const VENDOR_EQ: u16 = 0x001D;

// GET commands (vendor 0x000A)
pub const GET_FIRMWARE: u16 = 0x0309;
pub const GET_BATT_LEFT: u16 = 0x0306;
pub const GET_BATT_RIGHT: u16 = 0x0307;
pub const GET_BATT_CASE: u16 = 0x0323;
pub const GET_ANC_MODE: u16 = 0x0310;
pub const SET_ANC_MODE: u16 = 0x0311;
pub const GET_ANC_LEVEL: u16 = 0x0324;
pub const SET_ANC_LEVEL: u16 = 0x0325;
pub const GET_TRANSPARENCY_LEVEL: u16 = 0x034C;
pub const SET_TRANSPARENCY_LEVEL: u16 = 0x034D;
pub const GET_ANC_ENV_ADAPTIVE: u16 = 0x0364;
pub const SET_ANC_ENV_ADAPTIVE: u16 = 0x0365;
pub const GET_GAME_MODE: u16 = 0x030E;
pub const SET_GAME_MODE: u16 = 0x030F;
pub const GET_MULTIPOINT: u16 = 0x0314;
pub const SET_MULTIPOINT: u16 = 0x0315;
pub const GET_LDAC: u16 = 0x0338;
pub const SET_LDAC: u16 = 0x0339;
pub const GET_LE_AUDIO: u16 = 0x0335;
pub const SET_LE_AUDIO: u16 = 0x0334;
pub const GET_IN_EAR: u16 = 0x030C;
pub const SET_IN_EAR: u16 = 0x030D;
pub const GET_DISABLE_TOUCH: u16 = 0x0312;
pub const SET_DISABLE_TOUCH: u16 = 0x0313;
pub const GET_CUSTOM_KEYS: u16 = 0x03AA;
pub const SET_CUSTOM_KEYS: u16 = 0x03AB;
pub const CMD_RESET_CUSTOM_KEYS: u16 = 0x03AC;
pub const GET_PROMPT_VOLUME: u16 = 0x0319;
pub const SET_PROMPT_VOLUME: u16 = 0x0320;
pub const GET_AUTO_SHUTDOWN: u16 = 0x036E;
pub const SET_AUTO_SHUTDOWN: u16 = 0x036F;
pub const GET_VOICE_LANG: u16 = 0x0321;
pub const SET_VOICE_LANG: u16 = 0x0322;
pub const GET_DYNAMIC_EQ: u16 = 0x0331;
pub const SET_DYNAMIC_EQ: u16 = 0x0332;
pub const GET_DYNAMIC_BASS: u16 = 0x033C;
pub const SET_DYNAMIC_BASS: u16 = 0x033D;
pub const GET_MOVIE_MODE: u16 = 0x0327;
pub const SET_MOVIE_MODE: u16 = 0x0328;
pub const GET_SPATIAL_AUDIO: u16 = 0x0357;
pub const SET_SPATIAL_AUDIO: u16 = 0x0358;
pub const GET_PANORAMIC: u16 = 0x0329;
pub const SET_PANORAMIC: u16 = 0x0330;
pub const GET_DOLBY: u16 = 0x0348;
pub const SET_DOLBY: u16 = 0x0349;
pub const GET_ADAPTIVE_VOLUME: u16 = 0x0346;
pub const SET_ADAPTIVE_VOLUME: u16 = 0x0347;
pub const SET_FIND_LEFT: u16 = 0x033A;
pub const SET_FIND_RIGHT: u16 = 0x033B;
pub const GET_LIGHT_EFFECT: u16 = 0x030A;
pub const SET_LIGHT_EFFECT: u16 = 0x030B;
pub const GET_PRIVACY_MODE: u16 = 0x036B;
pub const SET_PRIVACY_MODE: u16 = 0x036C;
pub const GET_EARBUD_COLOUR: u16 = 0x0326;
pub const GET_PHONE_CODEC: u16 = 0x0377;

// EQ commands (vendor 0x001D)
pub const EQ_CMD_OLD: u16 = 0x0E01;
pub const EQ_CMD_NEW: u16 = 0x0E02;
pub const EQ_ACK: u16 = 0x0F81;

/// Safe GET commands to probe on connect.
/// Intentionally excludes 0x0317 (Connect classic) and 0x0352 (Known phones - private).
pub const PROBE_SEQUENCE: &[u16] = &[
    GET_FIRMWARE,
    GET_BATT_LEFT,
    GET_BATT_RIGHT,
    GET_BATT_CASE,
    GET_ANC_MODE,
    GET_ANC_LEVEL,
    GET_TRANSPARENCY_LEVEL,
    GET_ANC_ENV_ADAPTIVE,
    GET_GAME_MODE,
    GET_MULTIPOINT,
    GET_LDAC,
    GET_LE_AUDIO,
    GET_IN_EAR,
    GET_DISABLE_TOUCH,
    GET_CUSTOM_KEYS,
    GET_PROMPT_VOLUME,
    GET_AUTO_SHUTDOWN,
    GET_VOICE_LANG,
    GET_DYNAMIC_EQ,
    GET_DYNAMIC_BASS,
    GET_MOVIE_MODE,
    GET_SPATIAL_AUDIO,
    GET_PANORAMIC,
    GET_DOLBY,
    GET_ADAPTIVE_VOLUME,
    GET_LIGHT_EFFECT,
    GET_PRIVACY_MODE,
    GET_EARBUD_COLOUR,
    GET_PHONE_CODEC,
];

#[derive(Debug, Clone)]
pub struct GaiaFrame {
    pub version: u8,
    pub flags: u8,
    pub vendor: u16,
    pub command: u16,
    pub payload: Vec<u8>,
}

impl GaiaFrame {
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8 + self.payload.len());
        buf.push(SOF);
        buf.push(self.version);
        buf.push(self.flags);
        buf.push(self.payload.len() as u8);
        buf.extend_from_slice(&self.vendor.to_be_bytes());
        buf.extend_from_slice(&self.command.to_be_bytes());
        buf.extend_from_slice(&self.payload);
        buf
    }

    pub fn ctrl(command: u16, payload: &[u8]) -> Self {
        Self {
            version: TX_VERSION,
            flags: FLAGS,
            vendor: VENDOR_CTRL,
            command,
            payload: payload.to_vec(),
        }
    }

    pub fn eq(command: u16, payload: &[u8]) -> Self {
        Self {
            version: TX_VERSION,
            flags: FLAGS,
            vendor: VENDOR_EQ,
            command,
            payload: payload.to_vec(),
        }
    }

    pub fn to_hex(&self) -> String {
        self.encode()
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Stream reassembler based on the length byte at offset 3 (does not insist on version == 04).
#[derive(Default)]
pub struct FrameReader {
    buf: Vec<u8>,
}

impl FrameReader {
    pub fn push(&mut self, bytes: &[u8]) -> Vec<GaiaFrame> {
        self.buf.extend_from_slice(bytes);
        let mut frames = Vec::new();

        loop {
            // Sync to SOF = 0xFF
            if let Some(pos) = self.buf.iter().position(|&b| b == SOF) {
                if pos > 0 {
                    self.buf.drain(0..pos);
                }
            } else {
                self.buf.clear();
                break;
            }

            if self.buf.len() < 8 {
                break;
            }

            let len = self.buf[3] as usize;
            let total = 8 + len;
            if self.buf.len() < total {
                break;
            }

            let version = self.buf[1];
            let flags = self.buf[2];
            let vendor = u16::from_be_bytes([self.buf[4], self.buf[5]]);
            let command = u16::from_be_bytes([self.buf[6], self.buf[7]]);
            let payload = self.buf[8..total].to_vec();

            frames.push(GaiaFrame {
                version,
                flags,
                vendor,
                command,
                payload,
            });
            self.buf.drain(0..total);
        }

        frames
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EqBand {
    pub freq_hz: u16,
    pub gain_db: f32,
    pub q: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EqCommandKind {
    Old0E01,
    New0E02,
}

pub fn encode_eq_frames(
    cmd_kind: EqCommandKind,
    master_gain_db: f32,
    bands: &[EqBand],
) -> Vec<GaiaFrame> {
    let count = bands.len().min(15) as u8;
    let master_raw = (master_gain_db * 60.0).round().clamp(-32768.0, 32767.0) as i16;
    let cmd = match cmd_kind {
        EqCommandKind::Old0E01 => EQ_CMD_OLD,
        EqCommandKind::New0E02 => EQ_CMD_NEW,
    };

    bands
        .iter()
        .enumerate()
        .take(15)
        .map(|(i, band)| {
            let idx = (i + 1) as u8;
            let freq_raw = (band.freq_hz as u32 * 3).min(u16::MAX as u32) as u16;
            let gain_raw = (band.gain_db * 60.0).round().clamp(-32768.0, 32767.0) as i16;
            let q_raw = (band.q * 4096.0).round().clamp(1.0, u16::MAX as f32) as u16;

            let mut payload = Vec::with_capacity(10);
            match cmd_kind {
                EqCommandKind::Old0E01 => {
                    payload.push(((count & 0x0F) << 4) | (idx & 0x0F));
                }
                EqCommandKind::New0E02 => {
                    payload.push(count);
                    payload.push(idx);
                }
            }
            payload.extend_from_slice(&master_raw.to_be_bytes());
            payload.extend_from_slice(&freq_raw.to_be_bytes());
            payload.extend_from_slice(&gain_raw.to_be_bytes());
            payload.extend_from_slice(&q_raw.to_be_bytes());

            GaiaFrame::eq(cmd, &payload)
        })
        .collect()
}

/// Parses standard `[status, value]` reply; returns None if status != 0x00.
pub fn parse_status_u8(payload: &[u8]) -> Option<u8> {
    if payload.len() >= 2 {
        if payload[0] == 0x00 {
            Some(payload[1])
        } else {
            None
        }
    } else if payload.len() == 1 {
        Some(payload[0])
    } else {
        None
    }
}

/// Parses `(id, active)` pairs (used by 83 24 ANC level and 83 4C transparency level).
/// Drops leading `0x00` status byte if payload length is odd.
pub fn parse_level_pairs(payload: &[u8]) -> (Vec<u8>, Option<u8>) {
    let slice = if payload.len() % 2 == 1 && payload.first() == Some(&0x00) {
        &payload[1..]
    } else {
        payload
    };

    let mut supported = Vec::new();
    let mut active = None;
    for chunk in slice.chunks_exact(2) {
        let level = chunk[0];
        let is_active = chunk[1] == 0x01;
        supported.push(level);
        if is_active {
            active = Some(level);
        }
    }
    (supported, active)
}

/// Parses `83 AA` custom keys reply: `[status?, g1, a1, g2, a2, ...]`.
pub fn parse_custom_keys(payload: &[u8], expect_status_byte: bool) -> Vec<(u8, u8)> {
    let slice = if (expect_status_byte && payload.first() == Some(&0x00))
        || (payload.len() % 2 == 1 && payload.first() == Some(&0x00))
    {
        &payload[1..]
    } else {
        payload
    };

    slice
        .chunks_exact(2)
        .filter(|c| (1..=8).contains(&c[0]))
        .map(|c| (c[0], c[1]))
        .collect()
}

pub fn format_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}