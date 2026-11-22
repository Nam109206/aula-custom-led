#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Vec2};
use hidapi::{HidApi, HidDevice};
use serde::{Deserialize, Serialize};
use std::ffi::CString;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

// =============================================================================
// 1. LAYOUT CHUẨN 80 PHÍM AULA S75 PRO (Tổng chiều rộng 16.0u)
// =============================================================================
#[derive(Clone)]
struct KeyDef {
    index: usize,
    label: &'static str,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    cx: f32,
    cy: f32,
}

fn build_s75_layout() -> Vec<KeyDef> {
    let rows: &[(f32, &[(Option<&str>, f32)])] = &[
        // Row 0: 13 phím (góc trên phải dành cho LCD 1.14" và Núm xoay)
        (
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
        // Row 1: 15 phím
        (
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
        // Row 2: 15 phím
        (
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
        // Row 3: 14 phím
        (
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
        // Row 4: 13 phím
        (
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
                (Some("^"), 1.0),
            ],
        ),
        // Row 5: 10 phím
        (
            5.2,
            &[
                (Some("Ctrl"), 1.25),
                (Some("Win"), 1.25),
                (Some("Alt"), 1.25),
                (Some("Space"), 6.25),
                (Some("Alt"), 1.0),
                (Some("Fn"), 1.0),
                (Some("Ctrl"), 1.0),
                (Some("<"), 1.0),
                (Some("v"), 1.0),
                (Some(">"), 1.0),
            ],
        ),
    ];

    let mut keys = Vec::with_capacity(80);
    let mut idx = 0usize;
    for &(y, items) in rows {
        let mut x = 0.0f32;
        for &(opt_label, w) in items {
            if let Some(label) = opt_label {
                keys.push(KeyDef {
                    index: idx,
                    label,
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
// 2. CẤU HÌNH GIAO THỨC HID NGOÀI (protocol.json - Sửa không cần build lại)
// =============================================================================
#[derive(Serialize, Deserialize, Clone)]
struct ProtocolConfig {
    /// "64byte_chunks" hoặc "520byte_feature"
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
    fn config_path() -> PathBuf {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                return dir.join("protocol.json");
            }
        }
        PathBuf::from("protocol.json")
    }

    fn load_or_create() -> (Self, String) {
        let path = Self::config_path();
        if let Ok(raw) = fs::read_to_string(&path) {
            match serde_json::from_str::<ProtocolConfig>(&raw) {
                Ok(cfg) => return (cfg, format!("Đã nạp giao thức từ: {}", path.display())),
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
        (
            cfg,
            format!("Đã tạo file mẫu {} (có thể sửa header không cần compile lại).", path.display()),
        )
    }
}

#[derive(Serialize, Deserialize)]
struct PresetFile {
    keyboard: String,
    mode: String,
    effect: String,
    static_colors: Vec<[u8; 3]>,
    calibration: [f32; 3],
}

#[derive(Clone)]
struct ScannedDevice {
    vid: u16,
    pid: u16,
    interface: i32,
    usage_page: u16,
    usage: u16,
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
// 3. TRẠNG THÁI CHÍNH CỦA ỨNG DỤNG
// =============================================================================
#[derive(PartialEq, Eq, Clone, Copy)]
enum ControlMode {
    PerKeyPaint,
    LiveEffect,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum LiveEffect {
    RainbowWave,
    Breathing,
    DualWave,
    CenterPulse,
    FireSpark,
}

impl LiveEffect {
    fn name(self) -> &'static str {
        match self {
            LiveEffect::RainbowWave => "Rainbow Wave",
            LiveEffect::Breathing => "Breathing",
            LiveEffect::DualWave => "Dual Wave (Cyberpunk)",
            LiveEffect::CenterPulse => "Center Pulse",
            LiveEffect::FireSpark => "Fire Spark",
        }
    }

    fn from_name(s: &str) -> Self {
        match s {
            "Breathing" => LiveEffect::Breathing,
            "Dual Wave (Cyberpunk)" => LiveEffect::DualWave,
            "Center Pulse" => LiveEffect::CenterPulse,
            "Fire Spark" => LiveEffect::FireSpark,
            _ => LiveEffect::RainbowWave,
        }
    }
}

const KNOWN_AULA_VIDS: &[u16] = &[0x258A, 0x3554, 0x1A2C, 0x320F, 0x0C45];

struct S75App {
    keys: Vec<KeyDef>,
    static_colors: Vec<[u8; 3]>,
    current_frame: Vec<[u8; 3]>,

    mode: ControlMode,
    effect: LiveEffect,
    brush_color: [u8; 3],
    grad_left: [u8; 3],
    grad_right: [u8; 3],
    accent_color: [u8; 3],

    speed: f32,
    brightness: f32,
    cal_r: f32,
    cal_g: f32,
    cal_b: f32,

    start_time: Instant,
    last_send: Instant,

    // HID & Giao thức
    protocol: ProtocolConfig,
    hid_api: Option<HidApi>,
    scanned_devices: Vec<ScannedDevice>,
    selected_dev_idx: Option<usize>,
    connected_dev: Option<HidDevice>,
    connected_label: String,
    live_output: bool,
    show_safety_modal: bool,

    logs: Vec<String>,
}

impl S75App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_vietnamese_fonts(&cc.egui_ctx);
        apply_minimal_dark_theme(&cc.egui_ctx);

        let keys = build_s75_layout();
        let initial_colors = vec![[30, 144, 255]; keys.len()];
        let (protocol, proto_msg) = ProtocolConfig::load_or_create();

        let (hid_api, hid_msg) = match HidApi::new() {
            Ok(api) => (Some(api), "HID API khởi tạo thành công (chế độ Simulation an toàn).".to_string()),
            Err(e) => (None, format!("Không khởi tạo được HID API: {}", e)),
        };

        let mut app = Self {
            keys,
            static_colors: initial_colors.clone(),
            current_frame: initial_colors,
            mode: ControlMode::PerKeyPaint,
            effect: LiveEffect::RainbowWave,
            brush_color: [255, 64, 129],
            grad_left: [0, 240, 255],
            grad_right: [255, 0, 128],
            accent_color: [94, 129, 244],
            speed: 1.0,
            brightness: 1.0,
            cal_r: 1.0,
            cal_g: 1.0,
            cal_b: 1.0,
            start_time: Instant::now(),
            last_send: Instant::now(),
            protocol,
            hid_api,
            scanned_devices: Vec::new(),
            selected_dev_idx: None,
            connected_dev: None,
            connected_label: "Chưa kết nối".to_string(),
            live_output: false,
            show_safety_modal: false,
            logs: Vec::new(),
        };
        app.log(hid_msg);
        app.log(proto_msg);
        app
    }

    fn log(&mut self, msg: impl Into<String>) {
        self.logs.push(msg.into());
        if self.logs.len() > 80 {
            self.logs.remove(0);
        }
    }

    fn scan_hid(&mut self, only_aula: bool) {
        let Some(api) = self.hid_api.as_mut() else {
            self.log("[LỖI] HID API chưa sẵn sàng.");
            return;
        };
        if let Err(e) = api.refresh_devices() {
            self.log(format!("[LỖI] Không quét lại được USB HID: {}", e));
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
                usage: d.usage(),
                product: d.product_string().unwrap_or("Unknown HID").to_string(),
                path: d.path().to_owned(),
            });
        }
        self.scanned_devices = list;
        if self.scanned_devices.is_empty() {
            self.selected_dev_idx = None;
            self.log("Không thấy thiết bị phù hợp. Hãy cắm dây USB-C S75 rồi thử 'Quét tất cả HID'.");
        } else {
            self.selected_dev_idx = Some(0);
            let count = self.scanned_devices.len();
            self.log(format!(
                "Tìm thấy {} giao diện HID (ưu tiên chọn dòng có UP:0xFFxx):",
                count
            ));
            let preview_lines: Vec<String> = self
                .scanned_devices
                .iter()
                .take(8)
                .map(|d| format!("  -> {}", d.display_label()))
                .collect();
            for line in preview_lines {
                self.log(line);
            }
        }
    }

    fn connect_selected_hid(&mut self) {
        let Some(idx) = self.selected_dev_idx else {
            self.log("Hãy quét và chọn một cổng HID trước.");
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
                self.connected_label = info.display_label();
                self.live_output = false;
                let pkts = self.build_packets(&self.current_frame);
                let hex_preview: String = pkts
                    .first()
                    .map(|p| {
                        p.iter()
                            .take(16)
                            .map(|b| format!("{:02X}", b))
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .unwrap_or_default();
                self.log(format!("Đã mở kênh HID: {}", self.connected_label));
                self.log(format!(
                    "Gói mẫu #0 ({} bytes): {} ...",
                    pkts.first().map(|p| p.len()).unwrap_or(0),
                    hex_preview
                ));
            }
            Err(e) => {
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
        if !self.live_output {
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
            self.log(format!("[LỖI HID] Đã tự ngắt Live Output: {}", e));
        }
    }

    fn compute_frame(&self) -> Vec<[u8; 3]> {
        let t = self.start_time.elapsed().as_secs_f32() * self.speed;
        let raw: Vec<[u8; 3]> = match self.mode {
            ControlMode::PerKeyPaint => self.static_colors.clone(),
            ControlMode::LiveEffect => self
                .keys
                .iter()
                .map(|k| {
                    let nx = k.cx / 16.0;
                    let ny = k.cy / 6.2;
                    match self.effect {
                        LiveEffect::RainbowWave => {
                            let h = (nx - t * 0.25).rem_euclid(1.0);
                            hsv_to_rgb(h, 1.0, 1.0)
                        }
                        LiveEffect::Breathing => {
                            let v = ((t * 2.2).sin() + 1.0) * 0.5;
                            [
                                (self.brush_color[0] as f32 * v) as u8,
                                (self.brush_color[1] as f32 * v) as u8,
                                (self.brush_color[2] as f32 * v) as u8,
                            ]
                        }
                        LiveEffect::DualWave => {
                            let w = ((nx * 5.0 - t * 3.0).sin() + 1.0) * 0.5;
                            let r = w * 255.0;
                            let g = (1.0 - w) * 240.0;
                            let b = ((1.0 - w) + w * 0.5) * 255.0;
                            [r as u8, g as u8, b as u8]
                        }
                        LiveEffect::CenterPulse => {
                            let dx = nx - 0.5;
                            let dy = (ny - 0.5) * 0.7;
                            let dist = (dx * dx + dy * dy).sqrt();
                            let h = (dist * 1.2 - t * 0.4).rem_euclid(1.0);
                            hsv_to_rgb(h, 0.95, 1.0)
                        }
                        LiveEffect::FireSpark => {
                            let flicker = ((t * 6.0 + k.index as f32 * 1.7).sin() + 1.0) * 0.5;
                            let heat = ((1.0 - ny * 0.85) * (0.65 + 0.35 * flicker)).clamp(0.0, 1.0);
                            let r = (heat * 1.6).min(1.0) * 255.0;
                            let g = ((heat - 0.3).max(0.0) * 1.1).min(1.0) * 255.0;
                            [r as u8, g as u8, 0]
                        }
                    }
                })
                .collect(),
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

    fn apply_gradient(&mut self) {
        self.mode = ControlMode::PerKeyPaint;
        for k in &self.keys {
            let t = (k.cx / 16.0).clamp(0.0, 1.0);
            let r = (self.grad_left[0] as f32 * (1.0 - t) + self.grad_right[0] as f32 * t) as u8;
            let g = (self.grad_left[1] as f32 * (1.0 - t) + self.grad_right[1] as f32 * t) as u8;
            let b = (self.grad_left[2] as f32 * (1.0 - t) + self.grad_right[2] as f32 * t) as u8;
            self.static_colors[k.index] = [r, g, b];
        }
        self.log("Đã áp dụng màu Gradient toàn bàn phím.");
    }

    fn save_preset(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON Preset", &["json"])
            .set_file_name("s75_preset.json")
            .save_file()
        {
            let preset = PresetFile {
                keyboard: "AULA_S75_PRO".to_string(),
                mode: match self.mode {
                    ControlMode::PerKeyPaint => "Per-Key Paint".to_string(),
                    ControlMode::LiveEffect => "Live Effect".to_string(),
                },
                effect: self.effect.name().to_string(),
                static_colors: self.static_colors.clone(),
                calibration: [self.cal_r, self.cal_g, self.cal_b],
            };
            match serde_json::to_string_pretty(&preset)
                .map_err(|e| e.to_string())
                .and_then(|s| fs::write(&path, s).map_err(|e| e.to_string()))
            {
                Ok(_) => self.log(format!("Đã lưu preset: {}", path.display())),
                Err(e) => self.log(format!("[LỖI] Lưu preset thất bại: {}", e)),
            }
        }
    }

    fn load_preset(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON Preset", &["json"])
            .pick_file()
        {
            match fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|s| serde_json::from_str::<PresetFile>(&s).map_err(|e| e.to_string()))
            {
                Ok(p) => {
                    if p.static_colors.len() == self.keys.len() {
                        self.static_colors = p.static_colors;
                    }
                    self.mode = if p.mode == "Live Effect" {
                        ControlMode::LiveEffect
                    } else {
                        ControlMode::PerKeyPaint
                    };
                    self.effect = LiveEffect::from_name(&p.effect);
                    self.cal_r = p.calibration[0].clamp(0.0, 1.0);
                    self.cal_g = p.calibration[1].clamp(0.0, 1.0);
                    self.cal_b = p.calibration[2].clamp(0.0, 1.0);
                    self.log(format!("Đã nạp preset: {}", path.display()));
                }
                Err(e) => self.log(format!("[LỖI] Nạp preset thất bại: {}", e)),
            }
        }
    }
}

// =============================================================================
// 4. VÒNG LẶP VẼ GIAO DIỆN EGUI (20 FPS)
// =============================================================================
impl eframe::App for S75App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Giữ nhịp 20 FPS mượt mà và an toàn băng thông USB
        ctx.request_repaint_after(Duration::from_millis(50));

        self.current_frame = self.compute_frame();
        if self.live_output && self.last_send.elapsed() >= Duration::from_millis(50) {
            self.send_current_frame();
            self.last_send = Instant::now();
        }

        let accent = Color32::from_rgb(self.accent_color[0], self.accent_color[1], self.accent_color[2]);

        // --- SIDEBAR TRÁI ---
        egui::SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(290.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.heading(egui::RichText::new("S75 LED STUDIO").color(accent).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.color_edit_button_srgb(&mut self.accent_color)
                                .on_hover_text("Đổi màu nhấn giao diện");
                        });
                    });
                    ui.label(egui::RichText::new("Custom Per-Key RGB (80 phím)").weak().small());
                    ui.separator();

                    // Chế độ
                    ui.label(egui::RichText::new("CHẾ ĐỘ ĐIỀU KHIỂN").strong().small());
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.mode, ControlMode::PerKeyPaint, "Tô từng phím");
                        ui.selectable_value(&mut self.mode, ControlMode::LiveEffect, "Hiệu ứng động");
                    });
                    ui.add_space(8.0);

                    // Công cụ tô từng phím
                    ui.label(egui::RichText::new("CỌ VẼ PER-KEY").strong().small());
                    ui.horizontal(|ui| {
                        ui.label("Màu cọ:");
                        ui.color_edit_button_srgb(&mut self.brush_color);
                        if ui.button("Tô tất cả").clicked() {
                            self.mode = ControlMode::PerKeyPaint;
                            self.static_colors = vec![self.brush_color; self.keys.len()];
                        }
                        if ui.button("Tắt hết").clicked() {
                            self.mode = ControlMode::PerKeyPaint;
                            self.static_colors = vec![[0, 0, 0]; self.keys.len()];
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Gradient:");
                        ui.color_edit_button_srgb(&mut self.grad_left);
                        ui.label("->");
                        ui.color_edit_button_srgb(&mut self.grad_right);
                        if ui.button("Áp dụng").clicked() {
                            self.apply_gradient();
                        }
                    });
                    ui.add_space(8.0);
                    ui.separator();

                    // Hiệu ứng động
                    ui.label(egui::RichText::new("HIỆU ỨNG ĐỘNG (20 FPS)").strong().small());
                    egui::ComboBox::from_id_salt("effect_combo")
                        .selected_text(self.effect.name())
                        .width(260.0)
                        .show_ui(ui, |ui| {
                            for eff in [
                                LiveEffect::RainbowWave,
                                LiveEffect::Breathing,
                                LiveEffect::DualWave,
                                LiveEffect::CenterPulse,
                                LiveEffect::FireSpark,
                            ] {
                                if ui.selectable_value(&mut self.effect, eff, eff.name()).clicked() {
                                    self.mode = ControlMode::LiveEffect;
                                }
                            }
                        });
                    ui.add(egui::Slider::new(&mut self.speed, 0.2..=3.0).text("Tốc độ"));
                    ui.add(egui::Slider::new(&mut self.brightness, 0.0..=1.0).text("Độ sáng"));
                    ui.add_space(8.0);
                    ui.separator();

                    // Cân chỉnh màu RGB
                    ui.label(egui::RichText::new("CÂN MÀU RGB (CALIBRATION)").strong().small());
                    ui.add(egui::Slider::new(&mut self.cal_r, 0.0..=1.0).text("Red"));
                    ui.add(egui::Slider::new(&mut self.cal_g, 0.0..=1.0).text("Green"));
                    ui.add(egui::Slider::new(&mut self.cal_b, 0.0..=1.0).text("Blue"));
                    ui.add_space(8.0);
                    ui.separator();

                    // Preset & Protocol
                    ui.label(egui::RichText::new("PRESET & GIAO THỨC").strong().small());
                    ui.horizontal(|ui| {
                        if ui.button("Lưu Preset").clicked() {
                            self.save_preset();
                        }
                        if ui.button("Tải Preset").clicked() {
                            self.load_preset();
                        }
                    });
                    if ui.button("Nạp lại protocol.json").clicked() {
                        let (cfg, msg) = ProtocolConfig::load_or_create();
                        self.protocol = cfg;
                        self.log(msg);
                    }
                });
            });

        // --- KHU VỰC CHÍNH BÊN PHẢI ---
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("AULA S75 Pro — Live Preview");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new("Chuột trái / kéo: Tô màu  |  Chuột phải: Tắt LED phím")
                            .weak()
                            .small(),
                    );
                });
            });
            ui.add_space(6.0);

            // Canvas bàn phím 80 phím
            let desired_size = Vec2::new(ui.available_width().max(760.0), 335.0);
            let (canvas_rect, response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());
            let painter = ui.painter_at(canvas_rect);

            // Nền khung bàn phím
            painter.rect(
                canvas_rect,
                Rounding::same(10.0),
                Color32::from_rgb(16, 18, 24),
                Stroke::new(1.5, Color32::from_rgb(48, 54, 72)),
            );

            let pad_x = 24.0;
            let pad_y = 22.0;
            let unit = ((canvas_rect.width() - pad_x * 2.0) / 16.0)
                .min((canvas_rect.height() - pad_y * 2.0) / 6.2);
            let origin = Pos2::new(
                canvas_rect.center().x - unit * 8.0,
                canvas_rect.center().y - unit * 3.1,
            );
            let gap = 4.0;

            // Mô phỏng màn hình LCD & Núm xoay góc phải
            let lcd_rect = Rect::from_min_size(
                Pos2::new(origin.x + 13.85 * unit, origin.y),
                Vec2::new(1.05 * unit, unit - gap),
            );
            painter.rect(
                lcd_rect,
                Rounding::same(5.0),
                Color32::from_rgb(8, 20, 32),
                Stroke::new(1.5, Color32::from_rgb(78, 107, 138)),
            );
            painter.text(
                lcd_rect.center(),
                egui::Align2::CENTER_CENTER,
                "S75\nLCD",
                egui::FontId::monospace(10.0),
                Color32::from_rgb(80, 250, 123),
            );

            let knob_center = Pos2::new(origin.x + 15.45 * unit, origin.y + 0.46 * unit);
            painter.circle(
                knob_center,
                unit * 0.40,
                Color32::from_rgb(34, 38, 52),
                Stroke::new(2.0, Color32::from_rgb(122, 132, 158)),
            );
            painter.text(
                knob_center,
                egui::Align2::CENTER_CENTER,
                "VOL",
                egui::FontId::proportional(9.0),
                Color32::from_rgb(174, 182, 201),
            );

            // Kiểm tra tương tác tô màu phím
            let pointer_pos = response.interact_pointer_pos();
            let left_down = ctx.input(|i| i.pointer.primary_down());
            let right_down = ctx.input(|i| i.pointer.secondary_down());

            // Vẽ 80 phím
            for k in &self.keys {
                let key_rect = Rect::from_min_size(
                    Pos2::new(origin.x + k.x * unit, origin.y + k.y * unit),
                    Vec2::new(k.w * unit - gap, k.h * unit - gap),
                );

                if let Some(pos) = pointer_pos {
                    if key_rect.contains(pos) && (left_down || right_down) {
                        self.mode = ControlMode::PerKeyPaint;
                        self.static_colors[k.index] = if right_down {
                            [0, 0, 0]
                        } else {
                            self.brush_color
                        };
                    }
                }

                let [r, g, b] = self.current_frame[k.index];
                let lit = (r as u16 + g as u16 + b as u16) > 0;
                let fill = if lit {
                    Color32::from_rgb(r, g, b)
                } else {
                    Color32::from_rgb(28, 31, 42)
                };
                let stroke_c = if lit {
                    Color32::from_rgb(r, g, b)
                } else {
                    Color32::from_rgb(56, 62, 82)
                };
                let luma = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
                let text_c = if luma > 145.0 {
                    Color32::from_rgb(18, 18, 22)
                } else {
                    Color32::WHITE
                };

                painter.rect(key_rect, Rounding::same(5.0), fill, Stroke::new(1.5, stroke_c));
                painter.text(
                    key_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    k.label,
                    egui::FontId::proportional(11.0),
                    text_c,
                );
            }

            ui.add_space(10.0);
            ui.separator();

            // --- PANEL KẾT NỐI USB HID ---
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("USB HID BRIDGE:").strong());
                if ui.button("Quét thiết bị AULA").clicked() {
                    self.scan_hid(true);
                }
                if ui.button("Quét tất cả HID").clicked() {
                    self.scan_hid(false);
                }

                let selected_text = self
                    .selected_dev_idx
                    .and_then(|i| self.scanned_devices.get(i))
                    .map(|d| d.display_label())
                    .unwrap_or_else(|| "Chọn giao diện HID...".to_string());

                egui::ComboBox::from_id_salt("hid_dev_combo")
                    .selected_text(selected_text)
                    .width(290.0)
                    .show_ui(ui, |ui| {
                        for (i, d) in self.scanned_devices.iter().enumerate() {
                            ui.selectable_value(&mut self.selected_dev_idx, Some(i), d.display_label());
                        }
                    });

                if ui.button("Kết nối").clicked() {
                    self.connect_selected_hid();
                }

                let live_btn_text = if self.live_output {
                    "ĐANG GỬI LIVE (Bấm để dừng)"
                } else {
                    "Bật gửi Live xuống phím"
                };
                if ui.button(live_btn_text).clicked() {
                    if self.live_output {
                        self.live_output = false;
                        self.log("[LIVE OFF] Đã dừng gửi gói tin phần cứng.");
                    } else if self.connected_dev.is_none() {
                        self.log("Hãy quét và kết nối đúng cổng HID của S75 trước.");
                    } else {
                        self.show_safety_modal = true;
                    }
                }
            });

            ui.add_space(6.0);
            // Khung Log
            egui::Frame::none()
                .fill(Color32::from_rgb(12, 14, 19))
                .rounding(Rounding::same(6.0))
                .inner_margin(8.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .max_height(ui.available_height().max(110.0))
                        .show(ui, |ui| {
                            for line in &self.logs {
                                ui.label(
                                    egui::RichText::new(line)
                                        .monospace()
                                        .size(11.5)
                                        .color(Color32::from_rgb(166, 227, 161)),
                                );
                            }
                        });
                });
        });

        // Hộp thoại xác nhận an toàn trước khi bắn gói tin thật
        if self.show_safety_modal {
            egui::Window::new("Xác nhận an toàn HID")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label("Bạn đã cập nhật đúng packet_header của AULA S75 trong file protocol.json chưa?");
                    ui.label(
                        egui::RichText::new(
                            "Không nên gửi bừa gói tin chưa xác thực để tránh treo firmware bàn phím.",
                        )
                        .color(Color32::from_rgb(255, 184, 108)),
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Đã kiểm tra - Bật Live").clicked() {
                            self.live_output = true;
                            self.show_safety_modal = false;
                            self.log("[LIVE ON] Đang truyền LED 20 FPS xuống bàn phím...");
                        }
                        if ui.button("Hủy").clicked() {
                            self.show_safety_modal = false;
                        }
                    });
                });
        }
    }
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

fn apply_minimal_dark_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = Color32::from_rgb(20, 22, 30);
    visuals.panel_fill = Color32::from_rgb(24, 27, 36);
    visuals.window_rounding = Rounding::same(8.0);
    ctx.set_visuals(visuals);
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 690.0])
            .with_min_inner_size([980.0, 620.0]),
        ..Default::default()
    };
    eframe::run_native(
        "S75 LED Studio",
        options,
        Box::new(|cc| Ok(Box::new(S75App::new(cc)))),
    )
}
