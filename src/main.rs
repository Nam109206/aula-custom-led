#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::{self, Color32, ColorImage, Pos2, Rect, Rounding, Sense, Stroke, TextureHandle, Vec2};
use hidapi::{HidApi, HidDevice};
use image::codecs::gif::GifDecoder;
use image::AnimationDecoder;
use serde::{Deserialize, Serialize};
use std::ffi::CString;
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

// =============================================================================
// 1. LAYOUT CHUẨN 80 PHÍM AULA S75 PRO (Tổng chiều rộng 16.0u, 6 hàng)
// =============================================================================
#[derive(Clone)]
struct KeyDef {
    index: usize,
    label: &'static str,
    row: usize,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    cx: f32,
    cy: f32,
}

fn build_s75_layout() -> Vec<KeyDef> {
    let rows: &[(usize, f32, &[(Option<&str>, f32)])] = &[
        (
            0,
            0.0,
            &[
                (Some("Esc"), 1.0),
                (None, 0.25),
                (Some("F1"), 1.0),
                (Some("F2"), 1.0),
                (Some("F3"), 1.0),
                (Some("F4"), 1.0),
                (None, 0.25),
                (Some("F5"), 1.0),
                (Some("F6"), 1.0),
                (Some("F7"), 1.0),
                (Some("F8"), 1.0),
                (None, 0.25),
                (Some("F9"), 1.0),
                (Some("F10"), 1.0),
                (Some("F11"), 1.0),
                (Some("F12"), 1.0),
            ],
        ),
        (
            1,
            1.2,
            &[
                (Some("`"), 1.0),
                (Some("1"), 1.0),
                (Some("2"), 1.0),
                (Some("3"), 1.0),
                (Some("4"), 1.0),
                (Some("5"), 1.0),
                (Some("6"), 1.0),
                (Some("7"), 1.0),
                (Some("8"), 1.0),
                (Some("9"), 1.0),
                (Some("0"), 1.0),
                (Some("-"), 1.0),
                (Some("="), 1.0),
                (Some("Bksp"), 2.0),
                (Some("Home"), 1.0),
            ],
        ),
        (
            2,
            2.2,
            &[
                (Some("Tab"), 1.5),
                (Some("Q"), 1.0),
                (Some("W"), 1.0),
                (Some("E"), 1.0),
                (Some("R"), 1.0),
                (Some("T"), 1.0),
                (Some("Y"), 1.0),
                (Some("U"), 1.0),
                (Some("I"), 1.0),
                (Some("O"), 1.0),
                (Some("P"), 1.0),
                (Some("["), 1.0),
                (Some("]"), 1.0),
                (Some("\\"), 1.5),
                (Some("End"), 1.0),
            ],
        ),
        (
            3,
            3.2,
            &[
                (Some("Caps"), 1.75),
                (Some("A"), 1.0),
                (Some("S"), 1.0),
                (Some("D"), 1.0),
                (Some("F"), 1.0),
                (Some("G"), 1.0),
                (Some("H"), 1.0),
                (Some("J"), 1.0),
                (Some("K"), 1.0),
                (Some("L"), 1.0),
                (Some(";"), 1.0),
                (Some("'"), 1.0),
                (Some("Enter"), 2.25),
                (Some("Del"), 1.0),
            ],
        ),
        (
            4,
            4.2,
            &[
                (Some("LShift"), 2.25),
                (Some("Z"), 1.0),
                (Some("X"), 1.0),
                (Some("C"), 1.0),
                (Some("V"), 1.0),
                (Some("B"), 1.0),
                (Some("N"), 1.0),
                (Some("M"), 1.0),
                (Some(","), 1.0),
                (Some("."), 1.0),
                (Some("/"), 1.0),
                (Some("RShift"), 1.75),
                (Some("Up"), 1.0),
            ],
        ),
        (
            5,
            5.2,
            &[
                (Some("LCtrl"), 1.25),
                (Some("LWin"), 1.25),
                (Some("LAlt"), 1.25),
                (Some("Space"), 6.25),
                (Some("Fn"), 1.0),
                (Some("RCtrl"), 1.0),
                (None, 1.0),
                (Some("Left"), 1.0),
                (Some("Down"), 1.0),
                (Some("Right"), 1.0),
            ],
        ),
    ];

    let mut keys = Vec::with_capacity(80);
    let mut idx = 0usize;
    for &(row, y, items) in rows {
        let mut x = 0.0f32;
        for &(opt_label, w) in items {
            if let Some(label) = opt_label {
                keys.push(KeyDef {
                    index: idx,
                    label,
                    row,
                    x,
                    y,
                    w,
                    h: 1.0,
                    cx: x + w * 0.5,
                    cy: y + 0.5,
                });
                idx += 1;
            }
            x += w;
        }
    }
    keys
}

// =============================================================================
// 2. BỘ GIẢI MÃ GIF ANIMATION & LƯU TRỮ THƯ VIỆN
// =============================================================================
#[derive(Clone)]
struct GifFrameData {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
    delay_ms: u32,
}

#[derive(Clone)]
struct LoadedAnimation {
    name: String,
    file_path: PathBuf,
    frames: Vec<GifFrameData>,
    total_duration_ms: u64,
}

impl LoadedAnimation {
    fn from_path(path: &Path) -> Result<Self, String> {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("animation")
            .to_string();

        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        let mut frames = Vec::new();
        if ext == "gif" {
            let file = fs::File::open(path).map_err(|e| format!("Không mở được file: {}", e))?;
            let decoder =
                GifDecoder::new(BufReader::new(file)).map_err(|e| format!("Lỗi đọc GIF: {}", e))?;
            let decoded_frames = decoder
                .into_frames()
                .collect_frames()
                .map_err(|e| format!("Lỗi giải mã frame GIF: {}", e))?;

            for frame in decoded_frames {
                let (numer, denom) = frame.delay().numer_denom_ms();
                let raw_ms = if denom == 0 { 50 } else { numer / denom };
                let delay_ms = raw_ms.max(20);
                let buf = frame.into_buffer();
                frames.push(GifFrameData {
                    width: buf.width() as usize,
                    height: buf.height() as usize,
                    rgba: buf.into_raw(),
                    delay_ms,
                });
            }
        } else {
            let img = image::open(path)
                .map_err(|e| format!("Lỗi đọc ảnh: {}", e))?
                .to_rgba8();
            frames.push(GifFrameData {
                width: img.width() as usize,
                height: img.height() as usize,
                rgba: img.into_raw(),
                delay_ms: 100,
            });
        }

        if frames.is_empty() {
            return Err("File GIF không có frame nào.".to_string());
        }

        let total_duration_ms: u64 = frames.iter().map(|f| f.delay_ms as u64).sum::<u64>().max(20);
        Ok(Self {
            name,
            file_path: path.to_path_buf(),
            frames,
            total_duration_ms,
        })
    }

    fn frame_index_at(&self, elapsed_ms: u64) -> usize {
        if self.frames.len() <= 1 {
            return 0;
        }
        let pos = elapsed_ms % self.total_duration_ms;
        let mut acc = 0u64;
        for (i, f) in self.frames.iter().enumerate() {
            acc += f.delay_ms as u64;
            if pos < acc {
                return i;
            }
        }
        self.frames.len() - 1
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum GifFitMode {
    Stretch,
    Cover,
    Fit,
}

impl GifFitMode {
    fn label(self) -> &'static str {
        match self {
            GifFitMode::Stretch => "Stretch (Khớp toàn bàn phím)",
            GifFitMode::Cover => "Cover (Giữ tỷ lệ, lấp đầy)",
            GifFitMode::Fit => "Fit (Giữ nguyên khung hình)",
        }
    }
}

fn sample_gif_frame_for_keys(
    frame: &GifFrameData,
    keys: &[KeyDef],
    fit_mode: GifFitMode,
    mirror_x: bool,
    flip_y: bool,
) -> Vec<[u8; 3]> {
    let w = frame.width as f32;
    let h = frame.height as f32;
    if w <= 0.0 || h <= 0.0 {
        return vec![[0, 0, 0]; keys.len()];
    }

    let kb_aspect = 16.0 / 6.2;
    let img_aspect = w / h;

    keys.iter()
        .map(|k| {
            let mut nx = (k.cx / 16.0).clamp(0.0, 1.0);
            let mut ny = (k.cy / 6.2).clamp(0.0, 1.0);
            if mirror_x {
                nx = 1.0 - nx;
            }
            if flip_y {
                ny = 1.0 - ny;
            }

            let (u, v) = match fit_mode {
                GifFitMode::Stretch => (nx, ny),
                GifFitMode::Cover => {
                    if img_aspect > kb_aspect {
                        let scale = kb_aspect / img_aspect;
                        ((nx - 0.5) * scale + 0.5, ny)
                    } else {
                        let scale = img_aspect / kb_aspect;
                        (nx, (ny - 0.5) * scale + 0.5)
                    }
                }
                GifFitMode::Fit => {
                    if img_aspect > kb_aspect {
                        let scale = img_aspect / kb_aspect;
                        let vv = (ny - 0.5) * scale + 0.5;
                        (nx, vv)
                    } else {
                        let scale = kb_aspect / img_aspect;
                        let uu = (nx - 0.5) * scale + 0.5;
                        (uu, ny)
                    }
                }
            };

            if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
                return [0, 0, 0];
            }

            let cx_px = (u * (w - 1.0)).round() as isize;
            let cy_px = (v * (h - 1.0)).round() as isize;
            let mut acc_r = 0.0f32;
            let mut acc_g = 0.0f32;
            let mut acc_b = 0.0f32;
            let mut count = 0.0f32;

            for dy in -1..=1 {
                for dx in -1..=1 {
                    let px = (cx_px + dx).clamp(0, frame.width as isize - 1) as usize;
                    let py = (cy_px + dy).clamp(0, frame.height as isize - 1) as usize;
                    let off = (py * frame.width + px) * 4;
                    if off + 3 < frame.rgba.len() {
                        let a = frame.rgba[off + 3] as f32 / 255.0;
                        acc_r += frame.rgba[off] as f32 * a;
                        acc_g += frame.rgba[off + 1] as f32 * a;
                        acc_b += frame.rgba[off + 2] as f32 * a;
                        count += 1.0;
                    }
                }
            }

            if count > 0.0 {
                [
                    (acc_r / count).round().clamp(0.0, 255.0) as u8,
                    (acc_g / count).round().clamp(0.0, 255.0) as u8,
                    (acc_b / count).round().clamp(0.0, 255.0) as u8,
                ]
            } else {
                [0, 0, 0]
            }
        })
        .collect()
}

// =============================================================================
// 3. FONT BITMAP 5x7 CHO HIỆU ỨNG SCROLLING TEXT
// =============================================================================
fn glyph_5x7(ch: char) -> [u8; 7] {
    match ch.to_ascii_uppercase() {
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
        'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
        '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110],
        '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100],
        '?' => [0b01110, 0b10001, 0b00001, 0b00110, 0b00100, 0b00000, 0b00100],
        '-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
        '+' => [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000],
        ':' => [0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000],
        '.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100],
        '<' => [0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010],
        '>' => [0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000],
        _ => [0; 7],
    }
}

// =============================================================================
// 4. CẤU HÌNH GIAO THỨC HID NGOÀI (protocol.json) & PRESET
// =============================================================================
#[derive(Serialize, Deserialize, Clone)]
struct ProtocolConfig {
    protocol_mode: String,
    report_id: u8,
    packet_header: Vec<u8>,
    led_matrix_map: Vec<usize>,
}

impl Default for ProtocolConfig {
    fn default() -> Self {
        Self {
            protocol_mode: "64byte_chunks".to_string(),
            report_id: 0x00,
            packet_header: vec![0x04, 0x02, 0x00, 0x00],
            led_matrix_map: (0..80).collect(),
        }
    }
}

impl ProtocolConfig {
    fn app_dir() -> PathBuf {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                return dir.to_path_buf();
            }
        }
        PathBuf::from(".")
    }

    fn config_path() -> PathBuf {
        Self::app_dir().join("protocol.json")
    }

    fn animations_dir() -> PathBuf {
        Self::app_dir().join("animations")
    }

    fn load_or_create() -> (Self, String) {
        let path = Self::config_path();
        if let Ok(raw) = fs::read_to_string(&path) {
            match serde_json::from_str::<ProtocolConfig>(&raw) {
                Ok(cfg) => return (cfg, format!("Đã nạp giao thức từ {}", path.display())),
                Err(e) => {
                    return (
                        Self::default(),
                        format!("Lỗi đọc protocol.json ({}), dùng cấu hình mặc định.", e),
                    )
                }
            }
        }
        let cfg = Self::default();
        if let Ok(pretty) = serde_json::to_string_pretty(&cfg) {
            let _ = fs::write(&path, pretty);
        }
        (cfg, "Đã tạo protocol.json mặc định.".to_string())
    }
}

#[derive(Serialize, Deserialize)]
struct PresetFile {
    keyboard: String,
    static_colors: Vec<[u8; 3]>,
    calibration: [f32; 3],
}

#[derive(Clone)]
struct ScannedDevice {
    vid: u16,
    pid: u16,
    interface: i32,
    usage_page: u16,
    product: String,
    path: CString,
}

impl ScannedDevice {
    fn display_label(&self) -> String {
        format!(
            "{:04X}:{:04X} | IF:{} | UP:0x{:04X} | {}",
            self.vid, self.pid, self.interface, self.usage_page, self.product
        )
    }
}

// =============================================================================
// 5. TRẠNG THÁI ỨNG DỤNG
// =============================================================================
#[derive(PartialEq, Eq, Clone, Copy)]
enum NavTab {
    Play,
    Create,
    Apps,
    Settings,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum PlaySelection {
    Effect(BuiltInEffect),
    Animation(usize),
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum BuiltInEffect {
    Wave,
    Sweep,
    Bloom,
    ScrollingText,
    Breathing,
    FireSpark,
}

impl BuiltInEffect {
    const ALL: [BuiltInEffect; 6] = [
        BuiltInEffect::Wave,
        BuiltInEffect::Sweep,
        BuiltInEffect::Bloom,
        BuiltInEffect::ScrollingText,
        BuiltInEffect::Breathing,
        BuiltInEffect::FireSpark,
    ];

    fn name(self) -> &'static str {
        match self {
            BuiltInEffect::Wave => "Wave",
            BuiltInEffect::Sweep => "Sweep",
            BuiltInEffect::Bloom => "Bloom",
            BuiltInEffect::ScrollingText => "Scrolling text",
            BuiltInEffect::Breathing => "Breathing",
            BuiltInEffect::FireSpark => "Fire spark",
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum MiniAppMode {
    CyberpunkDual,
    MatrixRain,
    StarlightTwinkle,
}

impl MiniAppMode {
    fn name(self) -> &'static str {
        match self {
            MiniAppMode::CyberpunkDual => "Cyberpunk Dual Wave",
            MiniAppMode::MatrixRain => "Matrix Digital Rain",
            MiniAppMode::StarlightTwinkle => "Starlight Twinkle",
        }
    }
}

const KNOWN_AULA_VIDS: &[u16] = &[0x258A, 0x3554, 0x1A2C, 0x320F, 0x0C45];

struct S75App {
    keys: Vec<KeyDef>,
    static_colors: Vec<[u8; 3]>,
    current_frame: Vec<[u8; 3]>,

    nav_tab: NavTab,
    play_sel: PlaySelection,
    search_query: String,
    paused: bool,

    brightness: f32,
    speed: f32,
    hue: f32,

    scroll_message: String,
    scroll_cols_per_sec: f32,
    scroll_hue: f32,
    scroll_tint_each_letter: bool,
    scroll_top_row: usize,

    animations: Vec<LoadedAnimation>,
    gif_fit_mode: GifFitMode,
    gif_hue_shift: f32,
    gif_saturation: f32,
    gif_mirror_x: bool,
    gif_flip_y: bool,
    gif_preview_tex: Option<TextureHandle>,
    last_preview_key: (usize, usize),

    brush_color: [u8; 3],
    grad_left: [u8; 3],
    grad_right: [u8; 3],

    mini_app: MiniAppMode,

    cal_r: f32,
    cal_g: f32,
    cal_b: f32,
    target_fps: u32,

    start_time: Instant,
    last_send: Instant,

    protocol: ProtocolConfig,
    hid_api: Option<HidApi>,
    scanned_devices: Vec<ScannedDevice>,
    selected_dev_idx: Option<usize>,
    connected_dev: Option<HidDevice>,
    connected_device_name: String,
    live_output: bool,
    show_device_modal: bool,
    show_safety_modal: bool,
    status_warning: Option<String>,
    logs: Vec<String>,
}

impl S75App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_vietnamese_fonts(&cc.egui_ctx);
        apply_dark_theme(&cc.egui_ctx);

        let keys = build_s75_layout();
        let initial_colors = vec![[0, 175, 255]; keys.len()];
        let (protocol, proto_msg) = ProtocolConfig::load_or_create();
        let anim_dir = ProtocolConfig::animations_dir();
        let _ = fs::create_dir_all(&anim_dir);

        let (hid_api, hid_msg) = match HidApi::new() {
            Ok(api) => (Some(api), "HID API sẵn sàng.".to_string()),
            Err(e) => (None, format!("Không khởi tạo được HID API: {}", e)),
        };

        let mut app = Self {
            keys,
            static_colors: initial_colors.clone(),
            current_frame: initial_colors,
            nav_tab: NavTab::Play,
            play_sel: PlaySelection::Effect(BuiltInEffect::ScrollingText),
            search_query: String::new(),
            paused: false,
            brightness: 1.0,
            speed: 1.0,
            hue: 0.55,
            scroll_message: "HELLO".to_string(),
            scroll_cols_per_sec: 6.0,
            scroll_hue: 0.55,
            scroll_tint_each_letter: true,
            scroll_top_row: 1,
            animations: Vec::new(),
            gif_fit_mode: GifFitMode::Stretch,
            gif_hue_shift: 0.0,
            gif_saturation: 1.0,
            gif_mirror_x: false,
            gif_flip_y: false,
            gif_preview_tex: None,
            last_preview_key: (usize::MAX, usize::MAX),
            brush_color: [0, 180, 255],
            grad_left: [0, 240, 255],
            grad_right: [155, 48, 255],
            mini_app: MiniAppMode::CyberpunkDual,
            cal_r: 1.0,
            cal_g: 1.0,
            cal_b: 1.0,
            target_fps: 20,
            start_time: Instant::now(),
            last_send: Instant::now(),
            protocol,
            hid_api,
            scanned_devices: Vec::new(),
            selected_dev_idx: None,
            connected_dev: None,
            connected_device_name: "AULA S75 Pro".to_string(),
            live_output: false,
            show_device_modal: false,
            show_safety_modal: false,
            status_warning: None,
            logs: Vec::new(),
        };
        app.log(hid_msg);
        app.log(proto_msg);
        app.reload_animations_library();
        app
    }

    fn log(&mut self, msg: impl Into<String>) {
        self.logs.push(msg.into());
        if self.logs.len() > 100 {
            self.logs.remove(0);
        }
    }

    fn reload_animations_library(&mut self) {
        let dir = ProtocolConfig::animations_dir();
        let _ = fs::create_dir_all(&dir);
        let mut loaded = Vec::new();
        if let Ok(entries) = fs::read_dir(&dir) {
            let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            paths.sort();
            for p in paths {
                if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                    let ext_l = ext.to_ascii_lowercase();
                    if matches!(ext_l.as_str(), "gif" | "png" | "jpg" | "jpeg") {
                        match LoadedAnimation::from_path(&p) {
                            Ok(anim) => loaded.push(anim),
                            Err(e) => self.log(format!("[GIF] Bỏ qua {}: {}", p.display(), e)),
                        }
                    }
                }
            }
        }
        self.animations = loaded;
        self.last_preview_key = (usize::MAX, usize::MAX);
        self.log(format!("Thư viện Animations: {} file.", self.animations.len()));
    }

    fn import_gif_dialog(&mut self) {
        if let Some(paths) = rfd::FileDialog::new()
            .set_title("Chọn ảnh GIF hoặc ảnh tĩnh để thêm vào Animations")
            .add_filter("Animated GIF / Image", &["gif", "png", "jpg", "jpeg"])
            .pick_files()
        {
            let anim_dir = ProtocolConfig::animations_dir();
            let _ = fs::create_dir_all(&anim_dir);
            let mut imported_name = None;

            for src in paths {
                if let Some(file_name) = src.file_name() {
                    let dest = anim_dir.join(file_name);
                    if src != dest {
                        if let Err(e) = fs::copy(&src, &dest) {
                            self.status_warning = Some(format!("Không copy được file: {}", e));
                            continue;
                        }
                    }
                    imported_name = dest
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string());
                }
            }

            self.reload_animations_library();
            if let Some(target) = imported_name {
                if let Some(idx) = self.animations.iter().position(|a| a.name == target) {
                    self.nav_tab = NavTab::Play;
                    self.play_sel = PlaySelection::Animation(idx);
                    self.status_warning = None;
                    self.log(format!("Đã thêm và phát GIF: {}", target));
                }
            }
        }
    }

    fn remove_selected_animation(&mut self) {
        if let PlaySelection::Animation(idx) = self.play_sel {
            if let Some(anim) = self.animations.get(idx).cloned() {
                let _ = fs::remove_file(&anim.file_path);
                self.reload_animations_library();
                if self.animations.is_empty() {
                    self.play_sel = PlaySelection::Effect(BuiltInEffect::Wave);
                } else {
                    let next = idx.min(self.animations.len() - 1);
                    self.play_sel = PlaySelection::Animation(next);
                }
                self.log(format!("Đã xóa animation: {}", anim.name));
            }
        }
    }

    fn scan_hid(&mut self, only_aula: bool) {
        let Some(api) = self.hid_api.as_mut() else {
            self.log("[LỖI] HID API chưa sẵn sàng.");
            return;
        };
        if let Err(e) = api.refresh_devices() {
            self.log(format!("[LỖI] Quét USB HID thất bại: {}", e));
            return;
        }
        let mut list = Vec::new();
        for d in api.device_list() {
            let vid = d.vendor_id();
            if only_aula && !KNOWN_AULA_VIDS.contains(&vid) {
                continue;
            }
            list.push(ScannedDevice {
                vid,
                pid: d.product_id(),
                interface: d.interface_number(),
                usage_page: d.usage_page(),
                product: d.product_string().unwrap_or("AULA S75 HID").to_string(),
                path: d.path().to_owned(),
            });
        }
        self.scanned_devices = list;
        if self.scanned_devices.is_empty() {
            self.selected_dev_idx = None;
            self.log("Không thấy thiết bị phù hợp. Hãy cắm dây USB-C S75 rồi thử 'Quét tất cả HID'.");
        } else {
            self.selected_dev_idx = Some(0);
            self.log(format!("Tìm thấy {} cổng HID.", self.scanned_devices.len()));
        }
    }

    fn connect_selected_hid(&mut self) {
        let Some(idx) = self.selected_dev_idx else {
            return;
        };
        let Some(info) = self.scanned_devices.get(idx).cloned() else {
            return;
        };
        let Some(api) = self.hid_api.as_ref() else {
            return;
        };

        match api.open_path(&info.path) {
            Ok(dev) => {
                let _ = dev.set_blocking_mode(false);
                self.connected_dev = Some(dev);
                self.connected_device_name = format!("{} ({:04X}:{:04X})", info.product, info.vid, info.pid);
                self.status_warning = None;
                self.log(format!("Đã kết nối kênh HID: {}", info.display_label()));
            }
            Err(e) => {
                self.status_warning = Some(format!("Lỗi kết nối HID: {}", e));
                self.log(format!("[LỖI] Mở kênh HID thất bại: {}", e));
            }
        }
    }

    fn build_packets(&self, rgb_list: &[[u8; 3]]) -> Vec<Vec<u8>> {
        let mut ordered = Vec::with_capacity(240);
        for &hw_idx in &self.protocol.led_matrix_map {
            let rgb = rgb_list.get(hw_idx).copied().unwrap_or([0, 0, 0]);
            ordered.extend_from_slice(&rgb);
        }

        let mut packets = Vec::new();
        if self.protocol.protocol_mode == "520byte_feature" {
            let mut buf = vec![0u8; 520];
            buf[0] = self.protocol.report_id;
            for (i, &b) in self.protocol.packet_header.iter().enumerate() {
                if 1 + i < buf.len() {
                    buf[1 + i] = b;
                }
            }
            let start = 8usize;
            let copy_len = ordered.len().min(buf.len().saturating_sub(start));
            buf[start..start + copy_len].copy_from_slice(&ordered[..copy_len]);
            packets.push(buf);
        } else {
            let chunk_size = 56usize;
            let total_chunks = (ordered.len() + chunk_size - 1) / chunk_size;
            for (seq, chunk) in ordered.chunks(chunk_size).enumerate() {
                let mut buf = vec![0u8; 65];
                buf[0] = self.protocol.report_id;
                for (i, &b) in self.protocol.packet_header.iter().take(4).enumerate() {
                    buf[1 + i] = b;
                }
                buf[5] = seq as u8;
                buf[6] = total_chunks as u8;
                buf[7] = chunk.len() as u8;
                buf[8..8 + chunk.len()].copy_from_slice(chunk);
                packets.push(buf);
            }
        }
        packets
    }

    fn send_current_frame(&mut self) {
        if !self.live_output || self.paused {
            return;
        }
        let packets = self.build_packets(&self.current_frame);
        let is_feature = self.protocol.protocol_mode == "520byte_feature";
        let mut err_msg = None;

        if let Some(dev) = self.connected_dev.as_ref() {
            for pkt in &packets {
                let res = if is_feature {
                    dev.send_feature_report(pkt).map(|_| pkt.len())
                } else {
                    dev.write(pkt)
                };
                if let Err(e) = res {
                    err_msg = Some(e.to_string());
                    break;
                }
            }
        } else {
            self.live_output = false;
        }

        if let Some(e) = err_msg {
            self.live_output = false;
            self.status_warning = Some(format!("Đã ngắt Live HID do lỗi: {}", e));
            self.log(format!("[LỖI HID] {}", e));
        }
    }

    fn compute_frame(&mut self, ctx: &egui::Context) -> Vec<[u8; 3]> {
        if self.paused {
            return vec![[0, 0, 0]; self.keys.len()];
        }

        let elapsed_secs = self.start_time.elapsed().as_secs_f32();
        let raw: Vec<[u8; 3]> = match self.nav_tab {
            NavTab::Create => self.static_colors.clone(),
            NavTab::Apps => self.compute_mini_app_frame(elapsed_secs),
            NavTab::Play | NavTab::Settings => match self.play_sel {
                PlaySelection::Effect(eff) => self.compute_builtin_effect(eff, elapsed_secs),
                PlaySelection::Animation(anim_idx) => {
                    if let Some(anim) = self.animations.get(anim_idx).cloned() {
                        let elapsed_ms = (elapsed_secs * self.speed * 1000.0) as u64;
                        let f_idx = anim.frame_index_at(elapsed_ms);
                        let frame = &anim.frames[f_idx];

                        if self.last_preview_key != (anim_idx, f_idx) {
                            let img = ColorImage::from_rgba_unmultiplied(
                                [frame.width, frame.height],
                                &frame.rgba,
                            );
                            self.gif_preview_tex = Some(ctx.load_texture(
                                "gif_active_preview",
                                img,
                                egui::TextureOptions::LINEAR,
                            ));
                            self.last_preview_key = (anim_idx, f_idx);
                        }

                        let sampled = sample_gif_frame_for_keys(
                            frame,
                            &self.keys,
                            self.gif_fit_mode,
                            self.gif_mirror_x,
                            self.gif_flip_y,
                        );
                        sampled
                            .into_iter()
                            .map(|rgb| shift_hue_sat(rgb, self.gif_hue_shift, self.gif_saturation))
                            .collect()
                    } else {
                        self.compute_builtin_effect(BuiltInEffect::Wave, elapsed_secs)
                    }
                }
            },
        };

        raw.into_iter()
            .map(|[r, g, b]| {
                [
                    (r as f32 * self.brightness * self.cal_r).clamp(0.0, 255.0) as u8,
                    (g as f32 * self.brightness * self.cal_g).clamp(0.0, 255.0) as u8,
                    (b as f32 * self.brightness * self.cal_b).clamp(0.0, 255.0) as u8,
                ]
            })
            .collect()
    }

    fn compute_builtin_effect(&self, eff: BuiltInEffect, elapsed_secs: f32) -> Vec<[u8; 3]> {
        let t = elapsed_secs * self.speed;
        match eff {
            BuiltInEffect::Wave => self
                .keys
                .iter()
                .map(|k| {
                    let nx = k.cx / 16.0;
                    let h = (nx - t * 0.28).rem_euclid(1.0);
                    hsv_to_rgb(h, 1.0, 1.0)
                })
                .collect(),
            BuiltInEffect::Sweep => self
                .keys
                .iter()
                .map(|k| {
                    let nx = k.cx / 16.0;
                    let pos = (t * 0.45).rem_euclid(1.4) - 0.2;
                    let dist = (nx - pos).abs();
                    let v = (1.0 - dist * 4.5).clamp(0.0, 1.0);
                    hsv_to_rgb(self.hue, 0.9, v)
                })
                .collect(),
            BuiltInEffect::Bloom => self
                .keys
                .iter()
                .map(|k| {
                    let dx = k.cx / 16.0 - 0.5;
                    let dy = (k.cy / 6.2 - 0.5) * 0.75;
                    let dist = (dx * dx + dy * dy).sqrt();
                    let h = (self.hue + dist * 1.1 - t * 0.35).rem_euclid(1.0);
                    hsv_to_rgb(h, 0.95, 1.0)
                })
                .collect(),
            BuiltInEffect::ScrollingText => {
                let msg = if self.scroll_message.is_empty() {
                    " "
                } else {
                    self.scroll_message.as_str()
                };
                let chars: Vec<char> = msg.chars().collect();
                let char_stride = 6isize;
                let total_cols = (chars.len() as isize * char_stride + 16).max(16);
                let scroll_offset = (elapsed_secs * self.scroll_cols_per_sec) as isize;

                self.keys
                    .iter()
                    .map(|k| {
                        if k.row < self.scroll_top_row || k.row >= self.scroll_top_row + 7 {
                            return [0, 0, 0];
                        }
                        let glyph_row = k.row - self.scroll_top_row;
                        let kb_col = k.cx.floor() as isize;
                        let virtual_col = (kb_col + scroll_offset - 14).rem_euclid(total_cols);
                        if virtual_col < 0 {
                            return [0, 0, 0];
                        }
                        let char_idx = (virtual_col / char_stride) as usize;
                        let col_in_char = (virtual_col % char_stride) as usize;
                        if char_idx >= chars.len() || col_in_char >= 5 {
                            return [0, 0, 0];
                        }
                        let rows_bits = glyph_5x7(chars[char_idx]);
                        let bit = (rows_bits[glyph_row] >> (4 - col_in_char)) & 1;
                        if bit == 1 {
                            let h = if self.scroll_tint_each_letter {
                                (self.scroll_hue + char_idx as f32 * 0.14).rem_euclid(1.0)
                            } else {
                                self.scroll_hue
                            };
                            hsv_to_rgb(h, 0.92, 1.0)
                        } else {
                            [0, 0, 0]
                        }
                    })
                    .collect()
            }
            BuiltInEffect::Breathing => {
                let v = ((t * 2.2).sin() + 1.0) * 0.5;
                vec![hsv_to_rgb(self.hue, 0.95, v); self.keys.len()]
            }
            BuiltInEffect::FireSpark => self
                .keys
                .iter()
                .map(|k| {
                    let ny = k.cy / 6.2;
                    let flicker = ((t * 6.5 + k.index as f32 * 1.7).sin() + 1.0) * 0.5;
                    let heat = ((1.0 - ny * 0.85) * (0.65 + 0.35 * flicker)).clamp(0.0, 1.0);
                    let r = (heat * 1.6).min(1.0) * 255.0;
                    let g = ((heat - 0.3).max(0.0) * 1.1).min(1.0) * 255.0;
                    [r as u8, g as u8, 0]
                })
                .collect(),
        }
    }

    fn compute_mini_app_frame(&self, elapsed_secs: f32) -> Vec<[u8; 3]> {
        let t = elapsed_secs * self.speed;
        match self.mini_app {
            MiniAppMode::CyberpunkDual => self
                .keys
                .iter()
                .map(|k| {
                    let nx = k.cx / 16.0;
                    let w = ((nx * 5.0 - t * 3.0).sin() + 1.0) * 0.5;
                    [
                        (w * 255.0) as u8,
                        ((1.0 - w) * 235.0) as u8,
                        (((1.0 - w) + w * 0.55) * 255.0) as u8,
                    ]
                })
                .collect(),
            MiniAppMode::MatrixRain => self
                .keys
                .iter()
                .map(|k| {
                    let col = k.cx.floor();
                    let head = ((t * 5.0 + col * 1.9).rem_euclid(9.0)) - 1.5;
                    let dist = head - k.cy;
                    if (0.0..=3.2).contains(&dist) {
                        let v = 1.0 - dist / 3.2;
                        [
                            if dist < 0.55 { 180 } else { 0 },
                            (v * 255.0) as u8,
                            (v * 65.0) as u8,
                        ]
                    } else {
                        [0, 14, 4]
                    }
                })
                .collect(),
            MiniAppMode::StarlightTwinkle => self
                .keys
                .iter()
                .map(|k| {
                    let phase = (t * 2.6 + k.index as f32 * 2.37).sin();
                    let v = phase.max(0.0).powf(3.0);
                    hsv_to_rgb((self.hue + k.index as f32 * 0.01).rem_euclid(1.0), 0.5, v)
                })
                .collect(),
        }
    }

    fn apply_gradient(&mut self) {
        for k in &self.keys {
            let t = (k.cx / 16.0).clamp(0.0, 1.0);
            let r = (self.grad_left[0] as f32 * (1.0 - t) + self.grad_right[0] as f32 * t) as u8;
            let g = (self.grad_left[1] as f32 * (1.0 - t) + self.grad_right[1] as f32 * t) as u8;
            let b = (self.grad_left[2] as f32 * (1.0 - t) + self.grad_right[2] as f32 * t) as u8;
            self.static_colors[k.index] = [r, g, b];
        }
    }

    fn save_preset(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON Preset", &["json"])
            .set_file_name("s75_preset.json")
            .save_file()
        {
            let preset = PresetFile {
                keyboard: "AULA_S75_PRO".to_string(),
                static_colors: self.static_colors.clone(),
                calibration: [self.cal_r, self.cal_g, self.cal_b],
            };
            if let Ok(s) = serde_json::to_string_pretty(&preset) {
                let _ = fs::write(&path, s);
                self.log(format!("Đã lưu preset: {}", path.display()));
            }
        }
    }

    fn load_preset(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON Preset", &["json"])
            .pick_file()
        {
            if let Ok(raw) = fs::read_to_string(&path) {
                if let Ok(p) = serde_json::from_str::<PresetFile>(&raw) {
                    if p.static_colors.len() == self.keys.len() {
                        self.static_colors = p.static_colors;
                    }
                    self.cal_r = p.calibration[0].clamp(0.0, 1.0);
                    self.cal_g = p.calibration[1].clamp(0.0, 1.0);
                    self.cal_b = p.calibration[2].clamp(0.0, 1.0);
                    self.nav_tab = NavTab::Create;
                    self.log(format!("Đã nạp preset: {}", path.display()));
                }
            }
        }
    }
}

// =============================================================================
// 6. GIAO DIỆN CHÍNH (BRANDING: AULA S75)
// =============================================================================
impl eframe::App for S75App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let interval_ms = (1000 / self.target_fps.clamp(5, 60)) as u64;
        ctx.request_repaint_after(Duration::from_millis(interval_ms));

        self.current_frame = self.compute_frame(ctx);
        if self.live_output && self.last_send.elapsed() >= Duration::from_millis(interval_ms) {
            self.send_current_frame();
            self.last_send = Instant::now();
        }

        let sky_blue = Color32::from_rgb(122, 195, 245);
        let dark_text = Color32::from_rgb(18, 22, 28);
        let card_bg = Color32::from_rgb(31, 33, 39);
        let panel_bg = Color32::from_rgb(24, 26, 31);

        // ---------------------------------------------------------------------
        // TOP HEADER BAR (Logo aula s75 + Trạng thái thiết bị + Device + Pause)
        // ---------------------------------------------------------------------
        egui::TopBottomPanel::top("top_header")
            .exact_height(50.0)
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(18, 19, 23))
                    .inner_margin(egui::Margin::symmetric(14.0, 8.0))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(38, 41, 49))),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    // Badge logo aula s75
                    egui::Frame::none()
                        .fill(Color32::from_rgb(235, 238, 244))
                        .rounding(Rounding::same(3.0))
                        .inner_margin(egui::Margin::symmetric(8.0, 3.0))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new("aula s75")
                                    .color(Color32::from_rgb(18, 20, 26))
                                    .strong()
                                    .size(15.0),
                            );
                        });

                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 1.0;
                        ui.label(
                            egui::RichText::new(&self.connected_device_name)
                                .strong()
                                .size(13.0)
                                .color(Color32::from_rgb(230, 233, 240)),
                        );
                        ui.horizontal(|ui| {
                            let (dot_color, status_str) = if self.connected_dev.is_some() {
                                (Color32::from_rgb(76, 217, 100), "Connected")
                            } else {
                                (Color32::from_rgb(255, 184, 76), "Preview / Ready (USB)")
                            };
                            let (rect, _) = ui.allocate_exact_size(Vec2::new(8.0, 8.0), Sense::hover());
                            ui.painter().circle_filled(rect.center(), 4.0, dot_color);
                            ui.label(egui::RichText::new(status_str).size(11.0).weak());
                        });
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let pause_text = if self.paused {
                            "Resume lighting"
                        } else {
                            "Pause lighting"
                        };
                        let pause_btn = egui::Button::new(
                            egui::RichText::new(pause_text)
                                .color(if self.paused { Color32::WHITE } else { dark_text })
                                .strong()
                                .size(12.0),
                        )
                        .fill(if self.paused {
                            Color32::from_rgb(215, 85, 85)
                        } else {
                            sky_blue
                        })
                        .rounding(Rounding::same(3.0));

                        if ui.add_sized([110.0, 26.0], pause_btn).clicked() {
                            self.paused = !self.paused;
                        }

                        ui.add_space(6.0);
                        let dev_btn = egui::Button::new(
                            egui::RichText::new("Device")
                                .color(dark_text)
                                .strong()
                                .size(12.0),
                        )
                        .fill(Color32::from_rgb(225, 228, 235))
                        .rounding(Rounding::same(3.0));

                        if ui.add_sized([68.0, 26.0], dev_btn).clicked() {
                            self.show_device_modal = !self.show_device_modal;
                        }
                    });
                });
            });

        // ---------------------------------------------------------------------
        // LEFT SIDEBAR
        // ---------------------------------------------------------------------
        egui::SidePanel::left("main_sidebar")
            .resizable(false)
            .exact_width(195.0)
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(21, 23, 28))
                    .inner_margin(egui::Margin::symmetric(10.0, 10.0))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(38, 41, 49))),
            )
            .show(ctx, |ui| {
                for (tab, label) in [
                    (NavTab::Play, "Play"),
                    (NavTab::Create, "Create"),
                    (NavTab::Apps, "Apps"),
                    (NavTab::Settings, "Settings"),
                ] {
                    let active = self.nav_tab == tab;
                    let btn = egui::Button::new(
                        egui::RichText::new(label)
                            .size(13.5)
                            .strong()
                            .color(if active { dark_text } else { Color32::from_rgb(220, 224, 232) }),
                    )
                    .fill(if active { sky_blue } else { Color32::TRANSPARENT })
                    .rounding(Rounding::same(4.0));

                    if ui.add_sized([ui.available_width(), 34.0], btn).clicked() {
                        self.nav_tab = tab;
                    }
                    ui.add_space(2.0);
                }

                ui.add_space(10.0);

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    if ui
                        .add_sized(
                            [112.0, 24.0],
                            egui::Button::new(
                                egui::RichText::new("Library options")
                                    .color(dark_text)
                                    .size(11.5),
                            )
                            .fill(Color32::from_rgb(225, 228, 235))
                            .rounding(Rounding::same(3.0)),
                        )
                        .clicked()
                    {
                        self.reload_animations_library();
                    }

                    ui.add_space(4.0);
                    if ui
                        .add_sized(
                            [ui.available_width(), 30.0],
                            egui::Button::new(
                                egui::RichText::new("Import effect...")
                                    .color(dark_text)
                                    .strong()
                                    .size(12.5),
                            )
                            .fill(Color32::from_rgb(235, 238, 244))
                            .rounding(Rounding::same(3.0)),
                        )
                        .clicked()
                    {
                        self.import_gif_dialog();
                    }

                    ui.add_space(8.0);

                    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                        ui.label(egui::RichText::new("Effects").size(11.5).weak());
                        ui.add_space(2.0);
                        ui.add_sized(
                            [ui.available_width(), 24.0],
                            egui::TextEdit::singleline(&mut self.search_query)
                                .hint_text("Search effects"),
                        );
                        ui.add_space(6.0);

                        let query_lower = self.search_query.trim().to_ascii_lowercase();

                        egui::ScrollArea::vertical()
                            .max_height(ui.available_height())
                            .show(ui, |ui| {
                                for eff in BuiltInEffect::ALL {
                                    if !query_lower.is_empty()
                                        && !eff.name().to_ascii_lowercase().contains(&query_lower)
                                    {
                                        continue;
                                    }
                                    let selected = self.nav_tab == NavTab::Play
                                        && self.play_sel == PlaySelection::Effect(eff);

                                    let btn = egui::Button::new(
                                        egui::RichText::new(eff.name())
                                            .size(12.5)
                                            .color(if selected {
                                                dark_text
                                            } else {
                                                Color32::from_rgb(190, 195, 206)
                                            }),
                                    )
                                    .fill(if selected { sky_blue } else { Color32::TRANSPARENT })
                                    .rounding(Rounding::same(4.0));

                                    if ui.add_sized([ui.available_width(), 32.0], btn).clicked() {
                                        self.nav_tab = NavTab::Play;
                                        self.play_sel = PlaySelection::Effect(eff);
                                    }
                                }

                                ui.add_space(10.0);
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("Animations").size(11.5).weak());
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(
                                                egui::RichText::new(format!("({})", self.animations.len()))
                                                    .size(10.5)
                                                    .weak(),
                                            );
                                        },
                                    );
                                });
                                ui.add_space(4.0);

                                if self.animations.is_empty() {
                                    ui.label(
                                        egui::RichText::new("Bấm 'Import effect...'\nđể thêm file .GIF")
                                            .size(11.0)
                                            .weak(),
                                    );
                                } else {
                                    for i in 0..self.animations.len() {
                                        let anim_name = self.animations[i].name.clone();
                                        if !query_lower.is_empty()
                                            && !anim_name.to_ascii_lowercase().contains(&query_lower)
                                        {
                                            continue;
                                        }
                                        let selected = self.nav_tab == NavTab::Play
                                            && self.play_sel == PlaySelection::Animation(i);

                                        let btn = egui::Button::new(
                                            egui::RichText::new(&anim_name)
                                                .size(12.5)
                                                .color(if selected {
                                                    dark_text
                                                } else {
                                                    Color32::from_rgb(190, 195, 206)
                                                }),
                                        )
                                        .fill(if selected {
                                            sky_blue
                                        } else {
                                            Color32::TRANSPARENT
                                        })
                                        .rounding(Rounding::same(4.0));

                                        if ui.add_sized([ui.available_width(), 32.0], btn).clicked() {
                                            self.nav_tab = NavTab::Play;
                                            self.play_sel = PlaySelection::Animation(i);
                                        }
                                    }
                                }
                            });
                    });
                });
            });

        // ---------------------------------------------------------------------
        // MAIN PANEL BÊN PHẢI
        // ---------------------------------------------------------------------
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(panel_bg)
                    .inner_margin(egui::Margin::symmetric(16.0, 12.0)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if let Some(warn) = &self.status_warning {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Warning")
                                    .color(Color32::from_rgb(245, 175, 65))
                                    .strong()
                                    .size(13.0),
                            );
                            ui.label(egui::RichText::new(warn).size(12.0).weak());
                        });
                        ui.add_space(6.0);
                    }

                    // CARD 1: KEYBOARD PREVIEW
                    egui::Frame::none()
                        .fill(card_bg)
                        .rounding(Rounding::same(8.0))
                        .inner_margin(egui::Margin::symmetric(16.0, 12.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("Keyboard preview")
                                        .size(13.5)
                                        .color(Color32::from_rgb(220, 224, 232)),
                                );
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let badge = if self.live_output {
                                        "Live (Hardware)"
                                    } else if self.nav_tab == NavTab::Create {
                                        "Paint Mode (Left: Draw, Right: Off)"
                                    } else {
                                        "Live"
                                    };
                                    ui.label(egui::RichText::new(badge).size(12.0).weak());
                                });
                            });
                            ui.add_space(8.0);

                            let desired_w = ui.available_width().max(640.0);
                            let (canvas_rect, response) =
                                ui.allocate_exact_size(Vec2::new(desired_w, 290.0), Sense::click_and_drag());
                            let painter = ui.painter_at(canvas_rect);

                            painter.rect(
                                canvas_rect,
                                Rounding::same(6.0),
                                Color32::from_rgb(27, 29, 35),
                                Stroke::new(1.0, Color32::from_rgb(42, 45, 54)),
                            );

                            let unit = ((canvas_rect.width() - 48.0) / 16.0)
                                .min((canvas_rect.height() - 32.0) / 6.2);
                            let origin = Pos2::new(
                                canvas_rect.center().x - unit * 8.0,
                                canvas_rect.center().y - unit * 3.1,
                            );
                            let gap = 4.0;

                            let lcd_rect = Rect::from_min_size(
                                Pos2::new(origin.x + 13.85 * unit, origin.y),
                                Vec2::new(1.05 * unit, unit - gap),
                            );
                            painter.rect(
                                lcd_rect,
                                Rounding::same(4.0),
                                Color32::from_rgb(16, 22, 30),
                                Stroke::new(1.0, Color32::from_rgb(58, 66, 82)),
                            );
                            painter.text(
                                lcd_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "LCD",
                                egui::FontId::monospace(9.5),
                                Color32::from_rgb(120, 210, 160),
                            );

                            let knob_center = Pos2::new(origin.x + 15.45 * unit, origin.y + 0.46 * unit);
                            painter.circle(
                                knob_center,
                                unit * 0.38,
                                Color32::from_rgb(38, 42, 50),
                                Stroke::new(1.5, Color32::from_rgb(80, 88, 105)),
                            );

                            let pointer_pos = response.interact_pointer_pos();
                            let left_down = ctx.input(|i| i.pointer.primary_down());
                            let right_down = ctx.input(|i| i.pointer.secondary_down());

                            for k in &self.keys {
                                let key_rect = Rect::from_min_size(
                                    Pos2::new(origin.x + k.x * unit, origin.y + k.y * unit),
                                    Vec2::new(k.w * unit - gap, k.h * unit - gap),
                                );

                                if self.nav_tab == NavTab::Create {
                                    if let Some(pos) = pointer_pos {
                                        if key_rect.contains(pos) && (left_down || right_down) {
                                            self.static_colors[k.index] = if right_down {
                                                [0, 0, 0]
                                            } else {
                                                self.brush_color
                                            };
                                        }
                                    }
                                }

                                let [r, g, b] = self.current_frame[k.index];
                                let lit = (r as u16 + g as u16 + b as u16) > 8;
                                let fill = if lit {
                                    Color32::from_rgb(r, g, b)
                                } else {
                                    Color32::from_rgb(52, 56, 65)
                                };
                                let border = if lit {
                                    Color32::from_rgb(r, g, b)
                                } else {
                                    Color32::from_rgb(66, 70, 80)
                                };
                                let luma = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
                                let label_color = if luma > 145.0 {
                                    Color32::from_rgb(18, 20, 24)
                                } else {
                                    Color32::from_rgb(225, 228, 235)
                                };

                                painter.rect(
                                    key_rect,
                                    Rounding::same(5.0),
                                    fill,
                                    Stroke::new(1.0, border),
                                );
                                painter.text(
                                    key_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    k.label,
                                    egui::FontId::proportional(10.5),
                                    label_color,
                                );
                            }
                        });

                    ui.add_space(10.0);
                    if self.connected_dev.is_none() {
                        ui.label(
                            egui::RichText::new("Connect USB-C and click 'Device' at top-right to output live to AULA S75.")
                                .color(Color32::from_rgb(230, 165, 55))
                                .size(12.0),
                        );
                        ui.add_space(10.0);
                    }

                    // CARD 2: CONTROLS
                    egui::Frame::none()
                        .fill(card_bg)
                        .rounding(Rounding::same(8.0))
                        .inner_margin(egui::Margin::symmetric(18.0, 14.0))
                        .show(ui, |ui| {
                            match self.nav_tab {
                                NavTab::Play => self.ui_play_controls(ui, sky_blue),
                                NavTab::Create => self.ui_create_controls(ui),
                                NavTab::Apps => self.ui_apps_controls(ui, sky_blue),
                                NavTab::Settings => self.ui_settings_controls(ui, sky_blue),
                            }
                        });
                });
            });

        // POPUP DEVICE
        if self.show_device_modal {
            egui::Window::new("Device — AULA S75 USB HID")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.set_min_width(480.0);
                    ui.horizontal(|ui| {
                        if ui.button("Quét thiết bị AULA").clicked() {
                            self.scan_hid(true);
                        }
                        if ui.button("Quét tất cả HID").clicked() {
                            self.scan_hid(false);
                        }
                        if ui.button("Đóng").clicked() {
                            self.show_device_modal = false;
                        }
                    });
                    ui.add_space(6.0);

                    let selected_text = self
                        .selected_dev_idx
                        .and_then(|i| self.scanned_devices.get(i))
                        .map(|d| d.display_label())
                        .unwrap_or_else(|| "Chọn giao diện HID (ưu tiên UP:0xFFxx)...".to_string());

                    egui::ComboBox::from_id_salt("modal_hid_combo")
                        .selected_text(selected_text)
                        .width(460.0)
                        .show_ui(ui, |ui| {
                            for (i, d) in self.scanned_devices.iter().enumerate() {
                                ui.selectable_value(&mut self.selected_dev_idx, Some(i), d.display_label());
                            }
                        });

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Kết nối cổng đã chọn").clicked() {
                            self.connect_selected_hid();
                        }
                        let live_label = if self.live_output {
                            "ĐANG GỬI LIVE (Bấm để dừng)"
                        } else {
                            "Bật gửi Live xuống bàn phím"
                        };
                        if ui.button(live_label).clicked() {
                            if self.live_output {
                                self.live_output = false;
                                self.log("[LIVE OFF] Đã dừng gửi gói tin.");
                            } else if self.connected_dev.is_none() {
                                self.log("Hãy kết nối đúng cổng HID trước.");
                            } else {
                                self.show_safety_modal = true;
                            }
                        }
                    });
                });
        }

        if self.show_safety_modal {
            egui::Window::new("Xác nhận an toàn HID")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label("Bạn đã điền đúng packet_header của AULA S75 vào protocol.json chưa?");
                    ui.label(
                        egui::RichText::new("Không gửi bừa gói tin chưa xác thực để tránh treo firmware.")
                            .color(Color32::from_rgb(255, 184, 108)),
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Đã kiểm tra - Bật Live").clicked() {
                            self.live_output = true;
                            self.show_safety_modal = false;
                            self.log("[LIVE ON] Đang truyền LED xuống bàn phím...");
                        }
                        if ui.button("Hủy").clicked() {
                            self.show_safety_modal = false;
                        }
                    });
                });
        }
    }
}

impl S75App {
    fn ui_play_controls(&mut self, ui: &mut egui::Ui, accent: Color32) {
        ui.heading(egui::RichText::new("Effect controls").size(16.0));
        ui.label(
            egui::RichText::new("Changes are applied to your keyboard immediately.")
                .size(12.0)
                .weak(),
        );
        ui.add_space(12.0);

        match self.play_sel {
            PlaySelection::Effect(BuiltInEffect::ScrollingText) => {
                row_label(ui, "Message", |ui| {
                    ui.add_sized([280.0, 24.0], egui::TextEdit::singleline(&mut self.scroll_message));
                });
                slider_f32(ui, "Columns per second", &mut self.scroll_cols_per_sec, 1.0..=20.0, 2);
                slider_f32(ui, "Hue", &mut self.scroll_hue, 0.0..=1.0, 3);
                row_label(ui, "Tint each letter", |ui| {
                    ui.checkbox(&mut self.scroll_tint_each_letter, "");
                });
                slider_usize(ui, "Top row", &mut self.scroll_top_row, 0..=2);
                slider_f32(ui, "Brightness", &mut self.brightness, 0.0..=1.0, 3);
            }
            PlaySelection::Effect(eff) => {
                slider_f32(ui, "Speed", &mut self.speed, 0.1..=3.0, 2);
                if matches!(eff, BuiltInEffect::Sweep | BuiltInEffect::Bloom | BuiltInEffect::Breathing) {
                    slider_f32(ui, "Hue", &mut self.hue, 0.0..=1.0, 3);
                }
                slider_f32(ui, "Brightness", &mut self.brightness, 0.0..=1.0, 3);
            }
            PlaySelection::Animation(anim_idx) => {
                if let Some(anim) = self.animations.get(anim_idx).cloned() {
                    ui.horizontal(|ui| {
                        if let Some(tex) = &self.gif_preview_tex {
                            let max_h = 88.0;
                            let ratio = (anim.frames[0].width as f32 / anim.frames[0].height.max(1) as f32)
                                .clamp(0.4, 3.0);
                            ui.image((tex.id(), Vec2::new(max_h * ratio, max_h)));
                            ui.add_space(12.0);
                        }

                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(format!("GIF: {}", anim.name))
                                    .color(accent)
                                    .strong()
                                    .size(14.0),
                            );
                            ui.label(
                                egui::RichText::new(format!(
                                    "Kích thước: {}x{} px  |  Số frame: {}  |  Chu kỳ: {} ms",
                                    anim.frames[0].width,
                                    anim.frames[0].height,
                                    anim.frames.len(),
                                    anim.total_duration_ms
                                ))
                                .size(12.0)
                                .weak(),
                            );
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                if ui.button("+ Thêm GIF khác...").clicked() {
                                    self.import_gif_dialog();
                                }
                                if ui.button("Xóa GIF này").clicked() {
                                    self.remove_selected_animation();
                                }
                            });
                        });
                    });

                    ui.add_space(10.0);
                    slider_f32(ui, "Playback speed", &mut self.speed, 0.1..=3.0, 2);
                    slider_f32(ui, "Brightness", &mut self.brightness, 0.0..=1.0, 3);
                    slider_f32(ui, "Hue shift", &mut self.gif_hue_shift, 0.0..=1.0, 3);
                    slider_f32(ui, "Saturation", &mut self.gif_saturation, 0.0..=2.0, 2);

                    row_label(ui, "Fit mode", |ui| {
                        egui::ComboBox::from_id_salt("gif_fit_mode")
                            .selected_text(self.gif_fit_mode.label())
                            .width(240.0)
                            .show_ui(ui, |ui| {
                                for m in [GifFitMode::Stretch, GifFitMode::Cover, GifFitMode::Fit] {
                                    ui.selectable_value(&mut self.gif_fit_mode, m, m.label());
                                }
                            });
                    });

                    row_label(ui, "Mirror / Flip", |ui| {
                        ui.checkbox(&mut self.gif_mirror_x, "Mirror X");
                        ui.checkbox(&mut self.gif_flip_y, "Flip Y");
                    });
                }
            }
        }
    }

    fn ui_create_controls(&mut self, ui: &mut egui::Ui) {
        ui.heading(egui::RichText::new("Create — Per-Key Custom Lighting").size(16.0));
        ui.label(
            egui::RichText::new("Click or drag directly on keys above to paint. Right-click to turn a key off.")
                .size(12.0)
                .weak(),
        );
        ui.add_space(12.0);

        row_label(ui, "Brush color", |ui| {
            ui.color_edit_button_srgb(&mut self.brush_color);
            if ui.button("Fill all keys").clicked() {
                self.static_colors = vec![self.brush_color; self.keys.len()];
            }
            if ui.button("Clear all").clicked() {
                self.static_colors = vec![[0, 0, 0]; self.keys.len()];
            }
        });

        row_label(ui, "Gradient fill", |ui| {
            ui.color_edit_button_srgb(&mut self.grad_left);
            ui.label("->");
            ui.color_edit_button_srgb(&mut self.grad_right);
            if ui.button("Apply gradient").clicked() {
                self.apply_gradient();
            }
        });

        slider_f32(ui, "Brightness", &mut self.brightness, 0.0..=1.0, 3);

        row_label(ui, "Presets (.json)", |ui| {
            if ui.button("Save preset...").clicked() {
                self.save_preset();
            }
            if ui.button("Load preset...").clicked() {
                self.load_preset();
            }
        });
    }

    fn ui_apps_controls(&mut self, ui: &mut egui::Ui, accent: Color32) {
        ui.heading(egui::RichText::new("Apps — Reactive & Ambient Modes").size(16.0));
        ui.label(
            egui::RichText::new("Interactive lighting applets tailored for AULA S75 Pro.")
                .size(12.0)
                .weak(),
        );
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            for mode in [
                MiniAppMode::CyberpunkDual,
                MiniAppMode::MatrixRain,
                MiniAppMode::StarlightTwinkle,
            ] {
                let active = self.mini_app == mode;
                let btn = egui::Button::new(
                    egui::RichText::new(mode.name()).color(if active {
                        Color32::from_rgb(18, 22, 28)
                    } else {
                        Color32::WHITE
                    }),
                )
                .fill(if active { accent } else { Color32::from_rgb(45, 48, 58) });
                if ui.add(btn).clicked() {
                    self.mini_app = mode;
                }
            }
        });

        ui.add_space(10.0);
        slider_f32(ui, "Speed", &mut self.speed, 0.1..=3.0, 2);
        slider_f32(ui, "Hue", &mut self.hue, 0.0..=1.0, 3);
        slider_f32(ui, "Brightness", &mut self.brightness, 0.0..=1.0, 3);
    }

    fn ui_settings_controls(&mut self, ui: &mut egui::Ui, _accent: Color32) {
        ui.heading(egui::RichText::new("Settings & Color Calibration").size(16.0));
        ui.label(
            egui::RichText::new("Adjust per-channel RGB gain, frame rate, and HID protocol.")
                .size(12.0)
                .weak(),
        );
        ui.add_space(10.0);

        slider_f32(ui, "Red channel", &mut self.cal_r, 0.0..=1.0, 2);
        slider_f32(ui, "Green channel", &mut self.cal_g, 0.0..=1.0, 2);
        slider_f32(ui, "Blue channel", &mut self.cal_b, 0.0..=1.0, 2);

        row_label(ui, "Target FPS", |ui| {
            ui.add(egui::Slider::new(&mut self.target_fps, 10..=30).show_value(false));
            value_badge(ui, &format!("{}", self.target_fps));
        });

        row_label(ui, "Protocol file", |ui| {
            if ui.button("Reload protocol.json").clicked() {
                let (cfg, msg) = ProtocolConfig::load_or_create();
                self.protocol = cfg;
                self.log(msg);
            }
            if ui.button("Open Device settings").clicked() {
                self.show_device_modal = true;
            }
        });

        ui.add_space(8.0);
        egui::Frame::none()
            .fill(Color32::from_rgb(16, 18, 22))
            .rounding(Rounding::same(5.0))
            .inner_margin(8.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .max_height(90.0)
                    .show(ui, |ui| {
                        for line in &self.logs {
                            ui.label(
                                egui::RichText::new(line)
                                    .monospace()
                                    .size(11.0)
                                    .color(Color32::from_rgb(160, 215, 175)),
                            );
                        }
                    });
            });
    }
}

// =============================================================================
// 7. HELPER UI & XỬ LÝ MÀU
// =============================================================================
fn row_label(ui: &mut egui::Ui, label: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [170.0, 24.0],
            egui::Label::new(egui::RichText::new(label).size(12.5).weak()),
        );
        add_contents(ui);
    });
    ui.add_space(4.0);
}

fn slider_f32(
    ui: &mut egui::Ui,
    label: &str,
    val: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    decimals: usize,
) {
    row_label(ui, label, |ui| {
        ui.spacing_mut().slider_width = 250.0;
        ui.add(egui::Slider::new(val, range).show_value(false));
        ui.add_space(8.0);
        value_badge(ui, &format!("{:.1$}", *val, decimals));
    });
}

fn slider_usize(
    ui: &mut egui::Ui,
    label: &str,
    val: &mut usize,
    range: std::ops::RangeInclusive<usize>,
) {
    row_label(ui, label, |ui| {
        ui.spacing_mut().slider_width = 250.0;
        ui.add(egui::Slider::new(val, range).show_value(false));
        ui.add_space(8.0);
        value_badge(ui, &format!("{}", *val));
    });
}

fn value_badge(ui: &mut egui::Ui, text: &str) {
    egui::Frame::none()
        .fill(Color32::from_rgb(235, 238, 244))
        .rounding(Rounding::same(2.0))
        .inner_margin(egui::Margin::symmetric(8.0, 2.0))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(text)
                    .color(Color32::from_rgb(18, 20, 26))
                    .size(11.5),
            );
        });
}

fn shift_hue_sat([r, g, b]: [u8; 3], hue_shift: f32, sat_mul: f32) -> [u8; 3] {
    if hue_shift.abs() < 0.001 && (sat_mul - 1.0).abs() < 0.001 {
        return [r, g, b];
    }
    let (h, s, v) = rgb_to_hsv(r, g, b);
    let nh = (h + hue_shift).rem_euclid(1.0);
    let ns = (s * sat_mul).clamp(0.0, 1.0);
    hsv_to_rgb(nh, ns, v)
}

fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let rf = r as f32 / 255.0;
    let gf = g as f32 / 255.0;
    let bf = b as f32 / 255.0;
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let d = max - min;
    let v = max;
    let s = if max == 0.0 { 0.0 } else { d / max };
    let h = if d == 0.0 {
        0.0
    } else if max == rf {
        ((gf - bf) / d + if gf < bf { 6.0 } else { 0.0 }) / 6.0
    } else if max == gf {
        ((bf - rf) / d + 2.0) / 6.0
    } else {
        ((rf - gf) / d + 4.0) / 6.0
    };
    (h, s, v)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [u8; 3] {
    let i = (h * 6.0).floor() as i32;
    let f = h * 6.0 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match i.rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8]
}

fn configure_vietnamese_fonts(ctx: &egui::Context) {
    let candidates = [
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "C:\\Windows\\Fonts\\arial.ttf",
        "C:\\Windows\\Fonts\\tahoma.ttf",
    ];
    for path in candidates {
        if let Ok(bytes) = fs::read(path) {
            let mut fonts = egui::FontDefinitions::default();
            fonts
                .font_data
                .insert("vn_system".to_owned(), egui::FontData::from_owned(bytes));
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "vn_system".to_owned());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("vn_system".to_owned());
            ctx.set_fonts(fonts);
            break;
        }
    }
}

fn apply_dark_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = Color32::from_rgb(24, 26, 31);
    visuals.panel_fill = Color32::from_rgb(21, 23, 28);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(44, 47, 56);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(62, 66, 78);
    visuals.window_rounding = Rounding::same(6.0);
    ctx.set_visuals(visuals);
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1160.0, 680.0])
            .with_min_inner_size([960.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "aula s75 — LED Studio",
        options,
        Box::new(|cc| Ok(Box::new(S75App::new(cc)))),
    )
}
