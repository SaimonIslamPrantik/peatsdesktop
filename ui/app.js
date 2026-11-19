const post = (payload) => window.ipc.postMessage(JSON.stringify(payload));

let appState = null;
let pairedDevices = [];
let activeTab = "controls";
let pinned = false;
let paletteOverrideIdx = -1;
let ringState = { left: false, right: false };

const EXPRESSIVE_SEEDS = [
  "#9B82FF",
  "#FF7B6B",
  "#4DD0A7",
  "#5CB8FF",
];

const ANC_SCENE_LABELS = {
  0x11: "Adaptive / Normal",
  0x12: "Indoor",
  0x13: "Outdoor",
  0x14: "Traffic",
  0x15: "Adaptive+",
  0x16: "Anti-Wind",
  1: "Level 1",
  2: "Level 2",
  3: "Level 3",
  4: "Level 4",
  5: "Level 5",
};

const TRANS_LABELS = { 1: "Voice Enhance", 2: "Standard" };
const VOICE_LANGS = [[0, "EN"], [1, "ZH"], [2, "DE"], [3, "JA"], [4, "FR"], [5, "ES"], [6, "IT"]];
const AUTO_SHUTDOWN_MINS = [0, 5, 10, 30, 60, 120];

const GESTURE_NAMES = {
  1: "Single Tap L", 2: "Single Tap R",
  3: "Double Tap L", 4: "Double Tap R",
  5: "Triple Tap L", 6: "Triple Tap R",
  7: "Hold L (1.5s)", 8: "Hold R (1.5s)",
};

const ACTION_NAMES = {
  0: "None", 1: "Volume Up", 2: "Volume Down", 3: "Play / Pause",
  4: "Game Mode", 5: "Previous Track", 6: "Next Track",
  7: "ANC Cycle", 8: "Voice Assistant",
};

function hexToHsl(hex) {
  let r = parseInt(hex.slice(1, 3), 16) / 255;
  let g = parseInt(hex.slice(3, 5), 16) / 255;
  let b = parseInt(hex.slice(5, 7), 16) / 255;
  const max = Math.max(r, g, b), min = Math.min(r, g, b);
  let h = 0, s = 0, l = (max + min) / 2;
  if (max !== min) {
    const d = max - min;
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
    switch (max) {
      case r: h = (g - b) / d + (g < b ? 6 : 0); break;
      case g: h = (b - r) / d + 2; break;
      case b: h = (r - g) / d + 4; break;
    }
    h *= 60;
  }
  return [h, s * 100, l * 100];
}

function hsl(h, s, l) {
  return `hsl(${((h % 360) + 360) % 360}, ${Math.min(100, Math.max(0, s))}%, ${Math.min(100, Math.max(0, l))}%)`;
}

function applyMaterialTheme(themeObj) {
  const seed = paletteOverrideIdx >= 0
    ? EXPRESSIVE_SEEDS[paletteOverrideIdx]
    : (themeObj?.accent_hex || "#9B82FF");
  const dark = themeObj?.dark_mode ?? true;
  const [h, s] = hexToHsl(seed);
  const sat = Math.max(48, Math.min(82, s));
  const root = document.documentElement.style;

  if (dark) {
    root.setProperty("--md-primary", hsl(h, sat, 80));
    root.setProperty("--md-on-primary", hsl(h, sat, 22));
    root.setProperty("--md-primary-container", hsl(h, sat * 0.72, 31));
    root.setProperty("--md-on-primary-container", hsl(h, sat, 91));
    root.setProperty("--md-secondary-container", hsl(h + 15, 22, 29));
    root.setProperty("--md-on-secondary-container", hsl(h + 15, 32, 90));
    root.setProperty("--md-tertiary-container", hsl(h + 52, 34, 30));
    root.setProperty("--md-on-tertiary-container", hsl(h + 52, 55, 90));

    root.setProperty("--md-surface", hsl(h, 12, 8));
    root.setProperty("--md-surface-low", hsl(h, 12, 12));
    root.setProperty("--md-surface-high", hsl(h, 12, 16));
    root.setProperty("--md-surface-highest", hsl(h, 12, 22));
    root.setProperty("--md-on-surface", hsl(h, 14, 91));
    root.setProperty("--md-on-surface-var", hsl(h, 10, 74));
    root.setProperty("--md-outline-var", hsl(h, 10, 30));
  } else {
    root.setProperty("--md-primary", hsl(h, sat, 42));
    root.setProperty("--md-on-primary", "#ffffff");
    root.setProperty("--md-primary-container", hsl(h, sat, 90));
    root.setProperty("--md-on-primary-container", hsl(h, sat, 18));
    root.setProperty("--md-secondary-container", hsl(h + 15, 30, 88));
    root.setProperty("--md-on-secondary-container", hsl(h + 15, 30, 18));
    root.setProperty("--md-tertiary-container", hsl(h + 52, 48, 89));
    root.setProperty("--md-on-tertiary-container", hsl(h + 52, 48, 20));

    root.setProperty("--md-surface", hsl(h, 25, 98));
    root.setProperty("--md-surface-low", hsl(h, 22, 95));
    root.setProperty("--md-surface-high", hsl(h, 20, 91));
    root.setProperty("--md-surface-highest", hsl(h, 18, 86));
    root.setProperty("--md-on-surface", hsl(h, 16, 12));
    root.setProperty("--md-on-surface-var", hsl(h, 12, 34));
    root.setProperty("--md-outline-var", hsl(h, 12, 74));
  }
}

document.querySelectorAll(".nav-item").forEach((btn) => {
  btn.addEventListener("click", () => {
    activeTab = btn.dataset.tab;
    document.querySelectorAll(".nav-item").forEach((n) => n.classList.toggle("active", n === btn));
    document.querySelectorAll(".tab-pane").forEach((p) => p.classList.toggle("active", p.id === `tab-${activeTab}`));
  });
});

document.getElementById("pinBtn").addEventListener("click", (e) => {
  pinned = !pinned;
  e.currentTarget.setAttribute("aria-pressed", String(pinned));
  post({ cmd: "set_pinned", pinned });
});

document.getElementById("themeBtn").addEventListener("click", () => {
  paletteOverrideIdx++;
  if (paletteOverrideIdx >= EXPRESSIVE_SEEDS.length) paletteOverrideIdx = -1;
  applyMaterialTheme(window.__lastTheme);
  post({ cmd: "set_theme", palette_idx: paletteOverrideIdx });
});

document.getElementById("hideBtn").addEventListener("click", () => post({ cmd: "hide" }));
document.getElementById("demoBtn").addEventListener("click", () => post({ cmd: "start_demo" }));
document.getElementById("refreshBtn").addEventListener("click", () => post({ cmd: "scan_devices" }));
document.getElementById("disconnectBtn").addEventListener("click", () => post({ cmd: "disconnect" }));

document.querySelectorAll("#ancGroup .cbg-btn").forEach((btn) => {
  btn.addEventListener("click", () => {
    const mode = Number(btn.dataset.anc);
    if (appState) {
      appState.anc_mode = mode;
      renderNoiseControl(appState);
    }
    post({ cmd: "set_anc_mode", mode });
  });
});

document.getElementById("resetEqBtn").addEventListener("click", () => {
  if (!appState) return;
  const zero = appState.custom_slider_gains.map(() => 0);
  appState.custom_slider_gains = zero;
  renderEq(appState);
  post({ cmd: "apply_custom_eq", gains: zero });
});

document.getElementById("applyEqBtn").addEventListener("click", () => {
  if (!appState) return;
  post({ cmd: "apply_custom_eq", gains: appState.custom_slider_gains });
});

document.getElementById("findLBtn").addEventListener("click", () => {
  ringState.left = !ringState.left;
  document.getElementById("findLBtn").textContent = ringState.left ? "Stop Left" : "Ring Left";
  post({ cmd: "find_earbud", side: "left", on: ringState.left });
});

document.getElementById("findRBtn").addEventListener("click", () => {
  ringState.right = !ringState.right;
  document.getElementById("findRBtn").textContent = ringState.right ? "Stop Right" : "Ring Right";
  post({ cmd: "find_earbud", side: "right", on: ringState.right });
});

document.getElementById("promptVolSlider").addEventListener("input", (e) => {
  document.getElementById("promptVolVal").textContent = e.target.value;
});
document.getElementById("promptVolSlider").addEventListener("change", (e) => {
  post({ cmd: "set_u8", feature: "prompt_volume", value: Number(e.target.value) });
});

document.getElementById("resetKeysBtn").addEventListener("click", () => {
  post({ cmd: "reset_custom_keys" });
});

window.__onDevices = (devs) => {
  pairedDevices = devs || [];
  const list = document.getElementById("deviceList");
  list.innerHTML = "";
  if (!pairedDevices.length) {
    list.innerHTML = `<div class="device-item"><div><div class="dev-name">No paired Bluetooth devices</div><div class="dev-meta">Pair your SoundPEATS in Windows Settings first</div></div></div>`;
    return;
  }
  pairedDevices.forEach((d) => {
    const el = document.createElement("div");
    el.className = "device-item";
    el.innerHTML = `
      <div>
        <div class="dev-name">${escapeHtml(d.name)}</div>
        <div class="dev-meta">${d.address_str} · ${d.connected ? "Audio Connected" : "Paired"}</div>
      </div>
      <span class="dev-tag ${d.likely_soundpeats ? "peats" : ""}">${d.likely_soundpeats ? "SoundPEATS" : "BT"}</span>
    `;
    el.addEventListener("click", () => {
      post({ cmd: "connect", address: String(d.address), name: d.name });
    });
    list.appendChild(el);
  });
};

window.__onState = ({ state, theme, palette_idx }) => {
  appState = state;
  window.__lastTheme = theme;
  if (typeof palette_idx === "number") {
    paletteOverrideIdx = palette_idx;
  }
  applyMaterialTheme(theme);

  const isLive = state.connection_status === "connected" || state.connection_status === "demo";
  const isBusy = state.connection_status === "connecting" || state.connection_status === "probing";

  document.getElementById("wavyLoader").classList.toggle("hidden", !isBusy);
  document.getElementById("connectView").classList.toggle("hidden", isLive);
  document.getElementById("dashboardView").classList.toggle("hidden", !isLive);
  document.getElementById("bottomNav").classList.toggle("hidden", !isLive);

  document.getElementById("heroBadge").classList.toggle("active", isLive);
  document.getElementById("deviceTitle").textContent = isLive ? state.model.display_name : "PeatsDesktop";
  document.getElementById("deviceSub").textContent = state.status_message;

  const pill = document.getElementById("statusPill");
  pill.classList.toggle("live", isLive);
  pill.textContent = state.demo_mode
    ? "Demo"
    : state.connection_status === "connected"
    ? state.model.code
    : state.connection_status;

  if (!isLive) return;

  renderBatteries(state);
  renderNoiseControl(state);
  renderToggles(state);
  renderEq(state);
  renderDeviceTab(state);
};

function renderBatteries(st) {
  const setBatt = (id, barId, val) => {
    document.getElementById(id).textContent = val != null ? `${val}%` : "--%";
    document.getElementById(barId).style.width = `${val ?? 0}%`;
  };
  setBatt("battL", "battLBar", st.battery_left);
  setBatt("battR", "battRBar", st.battery_right);
  setBatt("battC", "battCBar", st.battery_case);
}

function renderNoiseControl(st) {
  const sup = new Set(st.supported);
  const ancCard = document.getElementById("ancCard");
  ancCard.classList.toggle("hidden", !sup.has("anc_mode"));
  if (!sup.has("anc_mode")) return;

  const names = { 0: "Off", 1: "ANC On", 2: "Passthrough", 3: "Adaptive", 4: "Weak" };
  document.getElementById("ancBadge").textContent = names[st.anc_mode] ?? "Off";

  document.querySelectorAll("#ancGroup .cbg-btn").forEach((b) => {
    b.classList.toggle("active", Number(b.dataset.anc) === st.anc_mode);
  });

  const scenesWrap = document.getElementById("ancScenesWrap");
  const scenesRow = document.getElementById("ancScenesRow");
  const showScenes = sup.has("anc_level") && st.anc_levels_supported.length > 0 && st.anc_mode === 1;
  scenesWrap.classList.toggle("hidden", !showScenes);
  if (showScenes) {
    scenesRow.innerHTML = "";
    st.anc_levels_supported.forEach((lvl) => {
      const c = document.createElement("button");
      c.className = `m3-chip ${st.anc_level === lvl ? "active" : ""}`;
      c.textContent = ANC_SCENE_LABELS[lvl] || `Scene ${lvl}`;
      c.addEventListener("click", () => post({ cmd: "set_anc_level", level: lvl }));
      scenesRow.appendChild(c);
    });
  }

  const transWrap = document.getElementById("transWrap");
  const transRow = document.getElementById("transRow");
  const showTrans = sup.has("transparency_level") && st.transparency_levels_supported.length > 0 && st.anc_mode === 2;
  transWrap.classList.toggle("hidden", !showTrans);
  if (showTrans) {
    transRow.innerHTML = "";
    st.transparency_levels_supported.forEach((lvl) => {
      const c = document.createElement("button");
      c.className = `m3-chip ${st.transparency_level === lvl ? "active" : ""}`;
      c.textContent = TRANS_LABELS[lvl] || `Mode ${lvl}`;
      c.addEventListener("click", () => post({ cmd: "set_transparency_level", level: lvl }));
      transRow.appendChild(c);
    });
  }
}

function renderToggles(st) {
  const sup = new Set(st.supported);
  const list = document.getElementById("togglesList");
  list.innerHTML = "";

  const defs = [
    ["ldac", "LDAC Hi-Res Audio", st.model.ldac_or_multipoint ? "Exclusive with Multipoint (restarts SPP)" : "High-bitrate codec advertisement", st.ldac],
    ["multipoint", "Dual Device Multipoint", st.model.ldac_or_multipoint ? "Exclusive with LDAC (restarts SPP)" : "Connect two sources simultaneously", st.multipoint],
    ["game_mode", "Low-Latency Game Mode", "Minimizes audio sync delay", st.game_mode],
    ["le_audio", "LE Audio (LC3)", "Next-gen Bluetooth audio", st.le_audio],
    ["disable_touch", "Disable Touch Controls", st.model.touch_inverted ? "Inverted polarity handled automatically" : "Prevent accidental taps", st.touch_disabled],
    ["in_ear", "In-Ear Detection", "Auto pause when removed", st.in_ear],
    ["anc_env_adaptive", "Environment Adaptive ANC", "Auto-adjusts cancellation strength", st.anc_env_adaptive],
    ["dynamic_eq", "Dynamic EQ", "Adaptive loudness compensation", st.dynamic_eq],
    ["dynamic_bass", "Dynamic Bass", "Low-frequency enhancement", st.dynamic_bass],
    ["movie_mode", "Movie Mode", "Cinema soundstage tuning", st.movie_mode],
    ["spatial_audio", "Spatial Audio", "3D head-stage rendering", st.spatial_audio],
    ["dolby", "Dolby Audio", "Dolby sound processing", st.dolby],
    ["adaptive_volume", "Adaptive Volume", "Matches ambient noise", st.adaptive_volume],
    ["light_effect", "Earbud Light Effect", "LED accent indicator", st.light_effect],
    ["privacy_mode", "Privacy Mode", "Limits microphone bleed", st.privacy_mode],
  ];

  const ldacExcludes = new Set(st.ldac ? st.model.ldac_excludes : []);
  document.getElementById("conflictHint").textContent = st.ldac && ldacExcludes.size ? "LDAC active" : "";

  defs.forEach(([key, title, sub, val]) => {
    if (!sup.has(key) && val == null) return;
    const blocked = ldacExcludes.has(key);
    const row = document.createElement("div");
    row.className = `toggle-row ${blocked ? "dimmed" : ""}`;
    row.innerHTML = `
      <div class="toggle-info">
        <div class="toggle-name">${title}</div>
        <div class="toggle-sub">${blocked ? "Disabled while LDAC is on" : sub}</div>
      </div>
      <button class="m3-switch ${val ? "on" : ""}" aria-checked="${!!val}"></button>
    `;
    row.querySelector(".m3-switch").addEventListener("click", () => {
      post({ cmd: "set_bool", feature: key, value: !val });
    });
    list.appendChild(row);
  });
}

function renderEq(st) {
  const cmdName = st.model.eq_command === "old0_e01" ? "0E01" : "0E02";
  document.getElementById("eqMeta").textContent = `${cmdName} · ${st.model.master_gain_db.toFixed(1)} dB`;

  const strip = document.getElementById("presetStrip");
  strip.innerHTML = "";
  st.model.presets.forEach((p) => {
    const b = document.createElement("button");
    b.className = `preset-pill ${st.active_preset === p.name ? "active" : ""}`;
    b.textContent = p.name;
    b.addEventListener("click", () => post({ cmd: "apply_preset", name: p.name }));
    strip.appendChild(b);
  });

  const freqs = st.model.custom_slider_freqs;
  const gains = st.custom_slider_gains;
  const wrap = document.getElementById("eqSliders");
  wrap.innerHTML = "";

  freqs.forEach((hz, i) => {
    const val = gains[i] ?? 0;
    const col = document.createElement("div");
    col.className = "eq-col";
    const lbl = hz >= 1000 ? `${(hz / 1000).toFixed(hz % 1000 === 0 ? 0 : 1)}k` : `${hz}`;
    col.innerHTML = `
      <div class="eq-db">${val > 0 ? "+" : ""}${val}</div>
      <div class="eq-range-wrap">
        <input class="eq-range" type="range" min="-6" max="6" step="1" value="${val}" />
      </div>
      <div class="eq-freq">${lbl}</div>
    `;
    const input = col.querySelector("input");
    const dbLbl = col.querySelector(".eq-db");
    input.addEventListener("input", () => {
      const v = Number(input.value);
      gains[i] = v;
      dbLbl.textContent = `${v > 0 ? "+" : ""}${v}`;
      drawEqCurve(gains);
    });
    wrap.appendChild(col);
  });

  drawEqCurve(gains);
}

function drawEqCurve(gains) {
  const svg = document.getElementById("eqSvg");
  const w = 360, h = 110, padX = 14, padY = 16;
  const n = gains.length || 1;
  const pts = gains.map((g, i) => {
    const x = padX + (i / Math.max(1, n - 1)) * (w - padX * 2);
    const y = h / 2 - (g / 6) * (h / 2 - padY);
    return [x, y];
  });

  let d = `M ${pts[0][0]} ${pts[0][1]}`;
  for (let i = 0; i < pts.length - 1; i++) {
    const [x0, y0] = pts[i];
    const [x1, y1] = pts[i + 1];
    const cx = (x0 + x1) / 2;
    d += ` C ${cx} ${y0}, ${cx} ${y1}, ${x1} ${y1}`;
  }

  const area = `${d} L ${pts[pts.length - 1][0]} ${h} L ${pts[0][0]} ${h} Z`;
  const dots = pts.map(([x, y]) => `<circle cx="${x}" cy="${y}" r="3.4" fill="var(--md-primary)" />`).join("");

  svg.innerHTML = `
    <defs>
      <linearGradient id="eqGrad" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0%" stop-color="var(--md-primary)" stop-opacity="0.38" />
        <stop offset="100%" stop-color="var(--md-primary)" stop-opacity="0.02" />
      </linearGradient>
    </defs>
    <line x1="0" y1="${h / 2}" x2="${w}" y2="${h / 2}" stroke="var(--md-outline-var)" stroke-dasharray="3 4" stroke-width="1" />
    <path d="${area}" fill="url(#eqGrad)" />
    <path d="${d}" fill="none" stroke="var(--md-primary)" stroke-width="2.5" stroke-linecap="round" />
    ${dots}
  `;
}

function renderDeviceTab(st) {
  const sup = new Set(st.supported);

  const pvWrap = document.getElementById("promptVolWrap");
  pvWrap.classList.toggle("hidden", !sup.has("prompt_volume"));
  if (sup.has("prompt_volume") && st.prompt_volume != null) {
    document.getElementById("promptVolSlider").value = st.prompt_volume;
    document.getElementById("promptVolVal").textContent = st.prompt_volume;
  }

  const vlWrap = document.getElementById("voiceLangWrap");
  const vlRow = document.getElementById("voiceLangRow");
  vlWrap.classList.toggle("hidden", !sup.has("voice_lang"));
  if (sup.has("voice_lang")) {
    vlRow.innerHTML = "";
    VOICE_LANGS.forEach(([id, lbl]) => {
      const c = document.createElement("button");
      c.className = `m3-chip ${st.voice_lang === id ? "active" : ""}`;
      c.textContent = lbl;
      c.addEventListener("click", () => post({ cmd: "set_u8", feature: "voice_lang", value: id }));
      vlRow.appendChild(c);
    });
  }

  const aoWrap = document.getElementById("autoOffWrap");
  const aoRow = document.getElementById("autoOffRow");
  aoWrap.classList.toggle("hidden", !sup.has("auto_shutdown"));
  if (sup.has("auto_shutdown")) {
    aoRow.innerHTML = "";
    AUTO_SHUTDOWN_MINS.forEach((m) => {
      const c = document.createElement("button");
      c.className = `m3-chip ${st.auto_shutdown === m ? "active" : ""}`;
      c.textContent = m === 0 ? "Never" : `${m}m`;
      c.addEventListener("click", () => post({ cmd: "set_u8", feature: "auto_shutdown", value: m }));
      aoRow.appendChild(c);
    });
  }

  const keysCard = document.getElementById("keysCard");
  const keysList = document.getElementById("keysList");
  keysCard.classList.toggle("hidden", !sup.has("custom_keys"));
  if (sup.has("custom_keys")) {
    keysList.innerHTML = "";
    st.custom_keys.forEach(([gesture, action]) => {
      const row = document.createElement("div");
      row.className = "key-row";
      const opts = Object.entries(ACTION_NAMES)
        .map(([k, v]) => `<option value="${k}" ${Number(k) === action ? "selected" : ""}>${v}</option>`)
        .join("");
      row.innerHTML = `<span>${GESTURE_NAMES[gesture] || `Gesture ${gesture}`}</span><select class="key-select">${opts}</select>`;
      row.querySelector("select").addEventListener("change", (e) => {
        post({ cmd: "set_custom_key", gesture, action: Number(e.target.value) });
      });
      keysList.appendChild(row);
    });
  }

  const logEl = document.getElementById("frameLog");
  logEl.innerHTML = "";
  [...st.frame_log].reverse().forEach((entry) => {
    const line = document.createElement("div");
    line.className = "log-line";
    line.innerHTML = `<span class="log-dir ${entry.dir}">${entry.dir}</span><span>${escapeHtml(entry.hex)} <em>(${escapeHtml(entry.label)})</em></span>`;
    logEl.appendChild(line);
  });
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
}

post({ cmd: "ready" });