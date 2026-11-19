use crate::protocol::{EqBand, EqCommandKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ChipFamily {
    Qualcomm,
    Wuqi,
    Jieli,
    Fota, // Bluetrum
    Bes,
    Actions,
    Smartlink,
    QcyWuqi,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EqMergeRule {
    OntoClassicBands,
    AppendAfterClassic,
    CompletePreset,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqPreset {
    pub name: String,
    pub bands: Vec<EqBand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelProfile {
    pub code: String,
    pub display_name: String,
    pub family: ChipFamily,
    pub eq_command: EqCommandKind,
    pub eq_merge: EqMergeRule,
    pub master_gain_db: f32,
    pub custom_slider_freqs: Vec<u16>,
    pub custom_q: f32,
    pub classic_bands: Vec<EqBand>,
    pub presets: Vec<EqPreset>,
    pub touch_inverted: bool,
    pub ldac_or_multipoint: bool,
    pub ldac_excludes: Vec<String>,
    pub adaptive_scene_is_11: bool,
}

const TOUCH_INVERTED_SUBSTRINGS: &[&str] = &[
    "bt5107",
    "bs519w2d",
    "air3deluxehsw53d",
    "air3pro",
    "bs390rb",
    "wqt7033",
    "qcc3040",
    "bs343rb",
    "bt5106-3040",
    "t28pro",
    "t28",
    "t57",
    "ats3019dk12",
    "opera03",
    "opera05",
    "hilistening",
    "freelite",
    "julirunfree",
];

const GENERIC_10_FREQS: &[u16] = &[31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
const BLUETRUM_8_FREQS: &[u16] = &[62, 125, 250, 500, 1000, 2000, 4000, 8000];

pub fn resolve_model(firmware_str: Option<&str>, bt_name: &str) -> ModelProfile {
    let code = firmware_str
        .and_then(|s| s.split('_').next())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("")
        .to_string();

    let upper_code = code.to_uppercase();
    let lower_code = code.to_lowercase();
    let touch_inverted = TOUCH_INVERTED_SUBSTRINGS
        .iter()
        .any(|pat| lower_code.contains(pat));

    match upper_code.as_str() {
        "S5" => build_h3_profile(touch_inverted),
        "S2" => build_air5_pro_plus_profile(touch_inverted),
        "S22" => build_c30_profile(touch_inverted),
        "T2" => build_clip_c10_profile(touch_inverted),
        _ => build_generic_profile(&code, bt_name, touch_inverted),
    }
}

fn b(freq_hz: u16, gain_db: f32, q: f32) -> EqBand {
    EqBand {
        freq_hz,
        gain_db,
        q,
    }
}

fn build_h3_profile(touch_inverted: bool) -> ModelProfile {
    // Exact captured H3 (S5) 10-band classic base curve (-2.0 dB master gain)
    let classic = vec![
        b(50, 3.5, 0.5),
        b(160, -6.0, 1.6),
        b(350, -2.6, 0.7),
        b(800, 3.7, 1.0),
        b(1600, 5.5, 1.0),
        b(2800, 6.5, 1.5),
        b(4600, -2.5, 1.8),
        b(8000, -3.5, 6.0),
        b(12000, 1.2, 1.5),
        b(17000, -0.2, 1.5),
    ];

    let apply_deltas = |d: [f32; 10]| -> Vec<EqBand> {
        classic
            .iter()
            .zip(d)
            .map(|(band, delta)| b(band.freq_hz, band.gain_db + delta, band.q))
            .collect()
    };

    let presets = vec![
        EqPreset {
            name: "SOUNDPEATS Classic".into(),
            bands: classic.clone(),
        },
        EqPreset {
            name: "Bass Boost".into(),
            bands: apply_deltas([3.0, 2.5, 1.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
        },
        EqPreset {
            name: "Bass Reduce".into(),
            bands: apply_deltas([-3.5, -2.5, -1.5, 0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.0]),
        },
        EqPreset {
            name: "Treble Boost".into(),
            bands: apply_deltas([0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 2.0, 3.0, 3.0, 2.0]),
        },
        EqPreset {
            name: "Pop".into(),
            bands: apply_deltas([1.5, 1.0, 0.0, -0.5, 1.0, 1.5, 1.0, 1.5, 1.0, 0.5]),
        },
        EqPreset {
            name: "Rock".into(),
            bands: apply_deltas([2.5, 1.5, -1.0, -1.0, 0.5, 1.5, 2.0, 2.5, 2.0, 1.0]),
        },
        EqPreset {
            name: "Electronic".into(),
            bands: apply_deltas([3.0, 2.0, 0.0, -1.0, 0.5, 1.0, 2.0, 3.0, 2.5, 1.5]),
        },
        EqPreset {
            name: "HIFI".into(),
            bands: apply_deltas([1.0, 0.5, 0.0, 0.5, 1.0, 1.0, 1.5, 2.0, 2.0, 1.5]),
        },
    ];

    ModelProfile {
        code: "S5".into(),
        display_name: "SOUNDPEATS H3".into(),
        family: ChipFamily::Qualcomm,
        eq_command: EqCommandKind::Old0E01,
        eq_merge: EqMergeRule::OntoClassicBands,
        master_gain_db: -2.0,
        custom_slider_freqs: classic.iter().map(|x| x.freq_hz).collect(),
        custom_q: 1.8,
        classic_bands: classic,
        presets,
        touch_inverted,
        ldac_or_multipoint: true,
        ldac_excludes: vec!["game_mode".into(), "multipoint".into()],
        adaptive_scene_is_11: true,
    }
}

fn build_air5_pro_plus_profile(touch_inverted: bool) -> ModelProfile {
    // Exact captured Air5 Pro+ (S2) 10-band base curve (-12.0 dB master gain)
    let classic = vec![
        b(32, 2.5, 1.0),
        b(65, 3.0, 1.5),
        b(180, -6.0, 1.2),
        b(500, 0.5, 1.5),
        b(800, 1.0, 1.5),
        b(1800, 8.0, 1.2),
        b(3600, 3.5, 3.0),
        b(5500, -5.0, 2.0),
        b(8000, -3.0, 3.5),
        b(12000, 8.0, 1.5),
    ];

    let apply_deltas = |d: [f32; 10]| -> Vec<EqBand> {
        classic
            .iter()
            .zip(d)
            .map(|(band, delta)| b(band.freq_hz, band.gain_db + delta, band.q))
            .collect()
    };

    let presets = vec![
        EqPreset {
            name: "SOUNDPEATS Classic".into(),
            bands: classic.clone(),
        },
        EqPreset {
            name: "Bass Boost".into(),
            bands: apply_deltas([3.0, 2.5, 1.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
        },
        EqPreset {
            name: "Bass Reduce".into(),
            bands: apply_deltas([-3.5, -2.5, -1.5, 0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.0]),
        },
        EqPreset {
            name: "Treble Boost".into(),
            bands: apply_deltas([0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 2.0, 2.5, 3.0, 2.5]),
        },
        EqPreset {
            name: "Pop".into(),
            bands: apply_deltas([1.0, 1.0, 0.0, 0.0, 1.0, 1.5, 1.0, 1.5, 1.0, 0.5]),
        },
        EqPreset {
            name: "HIFI".into(),
            bands: apply_deltas([1.0, 0.5, 0.0, 0.5, 1.0, 1.0, 1.5, 2.0, 2.0, 1.5]),
        },
    ];

    ModelProfile {
        code: "S2".into(),
        display_name: "SOUNDPEATS Air5 Pro+".into(),
        family: ChipFamily::Qualcomm,
        eq_command: EqCommandKind::Old0E01,
        eq_merge: EqMergeRule::OntoClassicBands,
        master_gain_db: -12.0,
        custom_slider_freqs: classic.iter().map(|x| x.freq_hz).collect(),
        custom_q: 1.8,
        classic_bands: classic,
        presets,
        touch_inverted,
        ldac_or_multipoint: false,
        ldac_excludes: vec!["game_mode".into()],
        adaptive_scene_is_11: true,
    }
}

fn build_c30_profile(touch_inverted: bool) -> ModelProfile {
    // Captured C30 (S22) 9-band base curve (-4.5 dB master gain)
    let classic = vec![
        b(20, -10.0, 1.5),
        b(160, -8.0, 1.4),
        b(300, -5.0, 0.8),
        b(800, -3.0, 1.0),
        b(1800, 3.1, 1.5),
        b(2800, 3.0, 1.5),
        b(5000, -4.0, 1.0),
        b(8000, 4.0, 1.2),
        b(16000, 4.0, 0.5),
    ];

    ModelProfile {
        code: "S22".into(),
        display_name: "SOUNDPEATS C30".into(),
        family: ChipFamily::Unknown,
        eq_command: EqCommandKind::New0E02,
        eq_merge: EqMergeRule::OntoClassicBands,
        master_gain_db: -4.5,
        custom_slider_freqs: classic.iter().map(|x| x.freq_hz).collect(),
        custom_q: 1.8,
        classic_bands: classic.clone(),
        presets: vec![EqPreset {
            name: "SOUNDPEATS Classic".into(),
            bands: classic,
        }],
        touch_inverted,
        ldac_or_multipoint: false,
        ldac_excludes: vec!["game_mode".into()],
        adaptive_scene_is_11: true,
    }
}

fn build_clip_c10_profile(touch_inverted: bool) -> ModelProfile {
    // Captured TRUEFREE Clip C10 (T2): 8 fixed bands + 8 user bands (-2.0 dB master)
    let fixed = vec![
        b(50, 0.0, 0.9),
        b(90, 0.1, 0.7),
        b(310, -4.0, 1.0),
        b(600, -1.0, 1.0),
        b(1550, 3.0, 1.5),
        b(2580, -10.0, 0.8),
        b(6200, -7.0, 2.5),
        b(7830, -8.0, 2.8),
    ];

    let classic_full: Vec<EqBand> = fixed
        .iter()
        .cloned()
        .chain(BLUETRUM_8_FREQS.iter().map(|&f| b(f, 0.0, 1.8)))
        .collect();

    ModelProfile {
        code: "T2".into(),
        display_name: "TRUEFREE Clip C10".into(),
        family: ChipFamily::Unknown,
        eq_command: EqCommandKind::New0E02,
        eq_merge: EqMergeRule::AppendAfterClassic,
        master_gain_db: -2.0,
        custom_slider_freqs: BLUETRUM_8_FREQS.to_vec(),
        custom_q: 1.8,
        classic_bands: fixed,
        presets: vec![EqPreset {
            name: "TRUEFREE Classic".into(),
            bands: classic_full,
        }],
        touch_inverted,
        ldac_or_multipoint: false,
        ldac_excludes: vec!["game_mode".into()],
        adaptive_scene_is_11: false,
    }
}

fn build_generic_profile(code: &str, bt_name: &str, touch_inverted: bool) -> ModelProfile {
    let upper = code.to_uppercase();
    let lower_name = bt_name.to_lowercase();

    let family = if matches!(upper.as_str(), "S27" | "S32" | "S5" | "S2")
        || upper.contains("QCC")
        || lower_name.contains("air5 pro")
        || lower_name.contains("air6 pro")
        || lower_name.contains("h3")
    {
        ChipFamily::Qualcomm
    } else if matches!(upper.as_str(), "S13" | "S19" | "S29") || upper.contains("BT5") {
        ChipFamily::Fota
    } else if upper == "S7" {
        ChipFamily::Bes
    } else if upper.contains("JIELI") || upper.starts_with("JL") {
        ChipFamily::Jieli
    } else if upper.contains("WUQI") || upper.starts_with("WQ") {
        ChipFamily::Wuqi
    } else if upper.starts_with("ATS") {
        ChipFamily::Actions
    } else {
        ChipFamily::Unknown
    };

    let eq_command = if family == ChipFamily::Qualcomm
        || matches!(upper.as_str(), "WQT7033" | "T57" | "ATS3019DK12" | "BS519W2D")
    {
        EqCommandKind::Old0E01
    } else {
        EqCommandKind::New0E02
    };

    let eq_merge = if family == ChipFamily::Qualcomm
        || matches!(
            upper.as_str(),
            "QCYTS04"
                | "SPTS04"
                | "ZKLXSPACEPRO"
                | "ZKLXAIR5LITE"
                | "S13"
                | "S15"
                | "S22"
                | "S29"
                | "ATS3019DK12"
                | "ZKLXBREEZY"
                | "T57"
                | "WQT7033"
                | "WUQIMINIHS"
        ) {
        EqMergeRule::OntoClassicBands
    } else {
        EqMergeRule::AppendAfterClassic
    };

    let freqs = if family == ChipFamily::Fota {
        BLUETRUM_8_FREQS.to_vec()
    } else {
        GENERIC_10_FREQS.to_vec()
    };

    let classic: Vec<EqBand> = freqs.iter().map(|&f| b(f, 0.0, 1.8)).collect();
    let ldac_excludes = match family {
        ChipFamily::Jieli => vec![
            "game_mode".into(),
            "spatial_audio".into(),
            "dolby".into(),
            "movie_mode".into(),
        ],
        ChipFamily::Bes => vec![
            "game_mode".into(),
            "spatial_audio".into(),
            "dolby".into(),
            "movie_mode".into(),
            "dynamic_eq".into(),
            "dynamic_bass".into(),
        ],
        ChipFamily::Fota => vec![
            "game_mode".into(),
            "spatial_audio".into(),
            "dolby".into(),
            "movie_mode".into(),
            "dynamic_bass".into(),
        ],
        _ => vec!["game_mode".into()],
    };

    let adaptive_scene_is_11 = matches!(upper.as_str(), "S15" | "S21" | "S22" | "S5" | "S2");

    ModelProfile {
        code: if code.is_empty() {
            "GENERIC".into()
        } else {
            code.into()
        },
        display_name: if bt_name.is_empty() {
            "SoundPEATS Device".into()
        } else {
            bt_name.into()
        },
        family,
        eq_command,
        eq_merge,
        master_gain_db: 0.0,
        custom_slider_freqs: freqs,
        custom_q: 1.8,
        classic_bands: classic.clone(),
        presets: vec![EqPreset {
            name: "Flat".into(),
            bands: classic,
        }],
        touch_inverted,
        ldac_or_multipoint: false,
        ldac_excludes,
        adaptive_scene_is_11,
    }
}

/// Computes the final EQ bands to send on the wire from classic base + user slider offsets (-6..+6 dB).
pub fn compose_custom_eq(profile: &ModelProfile, slider_gains: &[f32]) -> Vec<EqBand> {
    match profile.eq_merge {
        EqMergeRule::OntoClassicBands | EqMergeRule::CompletePreset => profile
            .classic_bands
            .iter()
            .enumerate()
            .map(|(i, base)| {
                let offset = slider_gains.get(i).copied().unwrap_or(0.0);
                EqBand {
                    freq_hz: base.freq_hz,
                    gain_db: base.gain_db + offset,
                    q: base.q,
                }
            })
            .collect(),
        EqMergeRule::AppendAfterClassic => {
            let mut out = if profile.code == "T2" {
                profile.classic_bands.clone()
            } else {
                Vec::new()
            };
            for (i, &freq) in profile.custom_slider_freqs.iter().enumerate() {
                let gain = slider_gains.get(i).copied().unwrap_or(0.0);
                out.push(EqBand {
                    freq_hz: freq,
                    gain_db: gain,
                    q: profile.custom_q,
                });
            }
            out
        }
    }
}