use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use procmod_overlay::{Color, Overlay, OverlayTarget};

use crate::{LiveDetectionPacket, TagOffset};

#[derive(Clone, Debug)]
pub struct OverlayVisualSettings {
    // Enemy
    pub enemy_color: [u8; 3],
    pub enemy_box_type: String,
    pub enemy_head_dot: bool,
    pub enemy_predict: bool,
    pub enemy_snapline: bool,
    pub enemy_show_conf: bool,
    pub enemy_show_name: bool,
    pub enemy_show_dist: bool,

    // Friendly
    pub friendly_color: [u8; 3],
    pub friendly_box_type: String,
    pub friendly_head_dot: bool,
    pub friendly_predict: bool,
    pub friendly_snapline: bool,
    pub friendly_show_conf: bool,
    pub friendly_show_name: bool,
    pub friendly_show_dist: bool,

    // Smoothing & Prediction
    pub visual_smoothing: bool,
    pub smoothing_factor: f32, // 0.05 .. 1.00 (1.0 = instant)
    pub zero_lag_lead: bool,
    pub predict_lead_ms: f32,   // 10.0 .. 100.0 (default ~35.0 ms)

    // Visual tags positions (offsets)
    pub tag_offsets: Vec<TagOffset>,

    // HUD
    pub hud_enabled: bool,
    pub hud_position: String,
    pub hud_font_size: f32,
    pub hud_show_fps: bool,
    pub hud_show_capture_latency: bool,
    pub hud_show_inference_latency: bool,
    pub hud_show_render_latency: bool,
    pub hud_show_total_latency: bool,

    // FOV Circle
    pub center_fov_size: f32,
    pub show_fov_circle: bool,
    pub fov_color: [u8; 3],
    pub fov_thickness: f32,
}

#[link(name = "user32")]
extern "system" {
    fn GetDesktopWindow() -> *mut std::ffi::c_void;
    fn GetSystemMetrics(nIndex: i32) -> i32;
    fn SetWindowDisplayAffinity(hWnd: *mut std::ffi::c_void, dwAffinity: u32) -> i32;
}

#[derive(Clone)]
struct TrackedTarget {
    cls: usize,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    vx: f32,
    vy: f32,
    conf: f32,
    last_seen: Instant,
}

pub struct D3dOverlayRunner {
    pub is_running: Arc<AtomicBool>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl D3dOverlayRunner {
    pub fn start(
        live_detection: Arc<Mutex<LiveDetectionPacket>>,
        settings: Arc<Mutex<OverlayVisualSettings>>,
    ) -> Self {
        let is_running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&is_running);

        let thread_handle = std::thread::spawn(move || {
            let desktop_hwnd = unsafe { GetDesktopWindow() } as isize;
            let screen_w = unsafe { GetSystemMetrics(0) } as f32;
            let screen_h = unsafe { GetSystemMetrics(1) } as f32;

            let mut overlay = match Overlay::new(OverlayTarget::Hwnd(desktop_hwnd)) {
                Ok(o) => o,
                Err(err) => {
                    eprintln!("[D3D11 OVERLAY ERROR] Failed to initialize hardware overlay: {:?}", err);
                    return;
                }
            };

            unsafe {
                let hwnd = overlay.hwnd() as *mut std::ffi::c_void;
                let aff = SetWindowDisplayAffinity(hwnd, 0x00000011);
                eprintln!("[D3D11 OVERLAY] Exclude from screen capture affinity applied (HWND: {:p}, result: {})!", hwnd, aff);
            }

            eprintln!("[D3D11 OVERLAY] Native Direct3D 11 Transparent Overlay started on Desktop HWND!");

            let mut last_render_ms = 0.2f32;
            let mut tracked_targets: Vec<TrackedTarget> = Vec::new();
            let mut last_frame_time = Instant::now();
            let mut last_processed_frame_id: u64 = 0;
            let mut last_detection_time = Instant::now();

            while running_clone.load(Ordering::Relaxed) {
                let frame_render_start = Instant::now();
                let _dt = last_frame_time.elapsed().as_secs_f32().clamp(0.0005, 0.1);
                last_frame_time = Instant::now();

                let packet = if let Ok(lock) = live_detection.lock() {
                    lock.clone()
                } else {
                    LiveDetectionPacket::default()
                };

                let cfg = if let Ok(lock) = settings.lock() {
                    lock.clone()
                } else {
                    continue;
                };

                // Обновление отслеживаемых целей: выполняется строго ОДИН раз на каждый НОВЫЙ кадр YOLO
                let is_new_detection = packet.frame_id != 0 && packet.frame_id != last_processed_frame_id;
                if is_new_detection {
                    last_processed_frame_id = packet.frame_id;
                    let detection_dt = last_detection_time.elapsed().as_secs_f32().clamp(0.002, 0.100);
                    last_detection_time = Instant::now();

                    let num_existing = tracked_targets.len();
                    let mut matched_indices = vec![false; num_existing];

                    for raw in &packet.targets {
                        let mut best_match: Option<(usize, f32)> = None;
                        for i in 0..num_existing {
                            let tracked = &tracked_targets[i];
                            if !matched_indices[i] && tracked.cls == raw.cls {
                                let dx = raw.x - tracked.x;
                                let dy = raw.y - tracked.y;
                                let dist = (dx * dx + dy * dy).sqrt();
                                if dist < 120.0 {
                                    if best_match.map_or(true, |(_, d)| dist < d) {
                                        best_match = Some((i, dist));
                                    }
                                }
                            }
                        }

                        if let Some((idx, _)) = best_match {
                            matched_indices[idx] = true;
                            let t = &mut tracked_targets[idx];
                            
                            // Расчёт физической скорости в пикселях/сек на основе реального времени между кадрами детекции
                            let mut raw_vx = (raw.x - t.x) / detection_dt;
                            let mut raw_vy = (raw.y - t.y) / detection_dt;

                            // Мёртвая зона для микро-дрожания нейросети (< 10 px/s)
                            if raw_vx.abs() < 10.0 { raw_vx = 0.0; }
                            if raw_vy.abs() < 10.0 { raw_vy = 0.0; }

                            // Ограничение максимальной физической скорости (защита от скачков)
                            raw_vx = raw_vx.clamp(-2500.0, 2500.0);
                            raw_vy = raw_vy.clamp(-2500.0, 2500.0);

                            // Сглаживание вектора скорости между кадрами
                            let v_alpha = 0.40;
                            t.vx = t.vx * (1.0 - v_alpha) + raw_vx * v_alpha;
                            t.vy = t.vy * (1.0 - v_alpha) + raw_vy * v_alpha;

                            // Сглаживание координат (Anti-Jitter EMA)
                            if !cfg.visual_smoothing || cfg.smoothing_factor >= 0.95 {
                                // Мгновенная привязка к детекции (0 мс задержки, абсолютный отклик)
                                t.x = raw.x;
                                t.y = raw.y;
                                t.w = raw.w;
                                t.h = raw.h;
                            } else {
                                // Прямой управляемый коэффициент EMA на каждый новый кадр
                                let alpha = cfg.smoothing_factor.clamp(0.05, 1.0);
                                t.x += (raw.x - t.x) * alpha;
                                t.y += (raw.y - t.y) * alpha;
                                t.w += (raw.w - t.w) * alpha;
                                t.h += (raw.h - t.h) * alpha;
                            }
                            t.conf = raw.conf;
                            t.last_seen = Instant::now();
                        } else {
                            // Новая цель
                            tracked_targets.push(TrackedTarget {
                                cls: raw.cls,
                                x: raw.x,
                                y: raw.y,
                                w: raw.w,
                                h: raw.h,
                                vx: 0.0,
                                vy: 0.0,
                                conf: raw.conf,
                                last_seen: Instant::now(),
                            });
                        }
                    }

                    // Удаляем цели, не обновлявшиеся более 150 мс
                    tracked_targets.retain(|t| t.last_seen.elapsed().as_millis() < 150);
                }

                if overlay.begin_frame().is_ok() {
                    // 1. Отрисовка круга FOV (если включен)
                    if cfg.show_fov_circle && cfg.center_fov_size > 0.0 {
                        let cx = screen_w / 2.0;
                        let cy = screen_h / 2.0;
                        let radius = cfg.center_fov_size / 2.0;
                        let fov_col = Color::rgba(cfg.fov_color[0], cfg.fov_color[1], cfg.fov_color[2], 180);
                        let step = 2.0 * std::f32::consts::PI / 48.0;
                        let thick = cfg.fov_thickness.max(1.0);
                        for i in 0..48 {
                            let a1 = step * i as f32;
                            let a2 = step * (i + 1) as f32;
                            overlay.line(
                                cx + radius * a1.cos(),
                                cy + radius * a1.sin(),
                                cx + radius * a2.cos(),
                                cy + radius * a2.sin(),
                                thick,
                                fov_col,
                            );
                        }
                    }

                    // 2. Отрисовка сглаженных и предиктивных целей
                    for target in &tracked_targets {
                        // Экстраполяция в реальном времени (Zero-Lag Lead):
                        // Компенсирует время, прошедшее с момента захвата кадра GPU и инференса,
                        // проецируя бокс вперёд вдоль вектора движения, чтобы он не отставал от бегущего врага
                        let time_since_detection = target.last_seen.elapsed().as_secs_f32().min(0.06);
                        let pipeline_latency_sec = ((packet.cap_ms + packet.infer_ms) / 1000.0).clamp(0.001, 0.035);

                        let speed_sq = target.vx * target.vx + target.vy * target.vy;
                        let (center_x, center_y) = if cfg.zero_lag_lead && speed_sq > 15.0 * 15.0 {
                            let lead_sec = (time_since_detection + pipeline_latency_sec).min(0.06);
                            let ext_x = target.x + target.vx * lead_sec;
                            let ext_y = target.y + target.vy * lead_sec;
                            (ext_x, ext_y)
                        } else {
                            (target.x, target.y)
                        };

                        // Ограничение по радиусу FOV по реальным координатам
                        if cfg.center_fov_size > 0.0 {
                            let cx = screen_w / 2.0;
                            let cy = screen_h / 2.0;
                            let radius = cfg.center_fov_size / 2.0;
                            let dx = center_x - cx;
                            let dy = center_y - cy;
                            if (dx * dx + dy * dy).sqrt() > radius {
                                continue;
                            }
                        }

                        let is_enemy = target.cls == 0;
                        let (rgb, box_type, head_dot, predict, snapline, show_name, show_conf, show_dist) = if is_enemy {
                            (
                                cfg.enemy_color,
                                &cfg.enemy_box_type,
                                cfg.enemy_head_dot,
                                cfg.enemy_predict,
                                cfg.enemy_snapline,
                                cfg.enemy_show_name,
                                cfg.enemy_show_conf,
                                cfg.enemy_show_dist,
                            )
                        } else {
                            (
                                cfg.friendly_color,
                                &cfg.friendly_box_type,
                                cfg.friendly_head_dot,
                                cfg.friendly_predict,
                                cfg.friendly_snapline,
                                cfg.friendly_show_name,
                                cfg.friendly_show_conf,
                                cfg.friendly_show_dist,
                            )
                        };

                        let col = Color::rgb(rgb[0], rgb[1], rgb[2]);
                        let glow_col = Color::rgba(rgb[0], rgb[1], rgb[2], 35);
                        let w = target.w;
                        let h = target.h;
                        let left = center_x - w / 2.0;
                        let top = center_y - h / 2.0;
                        let bottom = top + h;

                        // Позиция головы (верхние ~14% высоты тела)
                        let head_x = center_x;
                        let head_y = top + h * 0.14;

                        // А. Линия привязки (Snapline) снизу экрана к ногам противника
                        if snapline {
                            let snap_col = Color::rgba(rgb[0], rgb[1], rgb[2], 150);
                            overlay.line(screen_w / 2.0, screen_h, center_x, bottom, 1.4, snap_col);
                        }

                        // Б. Предикт упреждения при стрельбе (Lead Vector Ahead + Aim Marker)
                        if predict {
                            let speed_sq = target.vx * target.vx + target.vy * target.vy;
                            if speed_sq > 25.0 * 25.0 {
                                let lead_dt = (cfg.predict_lead_ms / 1000.0).clamp(0.010, 0.12);
                                let pred_head_x = head_x + target.vx * lead_dt;
                                let pred_head_y = head_y + target.vy * lead_dt;
                                let pred_col = Color::rgb(255, 195, 18);

                                // Направляющая стрелка упреждения для стрельбы на опережение
                                overlay.line(head_x, head_y, pred_head_x, pred_head_y, 1.8, pred_col);
                                overlay.circle_filled(pred_head_x, pred_head_y, 3.2, pred_col);
                                overlay.circle(pred_head_x, pred_head_y, 5.5, Color::rgba(255, 195, 18, 160));
                            }
                        }

                        // В. Точка головы (Head Aim Target & Tactical Cross)
                        if head_dot {
                            // Прицельный круг
                            overlay.circle(head_x, head_y, 6.5, col);
                            // 4 тактические риски
                            overlay.line(head_x - 9.0, head_y, head_x - 4.5, head_y, 1.2, col);
                            overlay.line(head_x + 4.5, head_y, head_x + 9.0, head_y, 1.2, col);
                            overlay.line(head_x, head_y - 9.0, head_x, head_y - 4.5, 1.2, col);
                            overlay.line(head_x, head_y + 4.5, head_x, head_y + 9.0, 1.2, col);
                            // Центральная неоновая точка
                            overlay.circle_filled(head_x, head_y, 3.2, col);
                            overlay.circle_filled(head_x, head_y, 1.2, Color::WHITE);
                        }

                        // Г. Bounding Box
                        match box_type.as_str() {
                            "2D Box" => {
                                overlay.rect(left, top, w, h, col);
                                overlay.rect(left - 1.0, top - 1.0, w + 2.0, h + 2.0, Color::rgba(16, 18, 24, 180));
                            }
                            "Corner Box" => {
                                let corner_len = (w.min(h) * 0.22).clamp(8.0, 24.0);
                                let shadow = Color::rgba(16, 18, 24, 180);

                                // Top-Left
                                overlay.line(left, top, left + corner_len, top, 2.0, col);
                                overlay.line(left, top, left, top + corner_len, 2.0, col);
                                overlay.line(left + 1.0, top + 1.0, left + corner_len, top + 1.0, 1.0, shadow);
                                overlay.line(left + 1.0, top + 1.0, left + 1.0, top + corner_len, 1.0, shadow);

                                // Top-Right
                                overlay.line(left + w, top, left + w - corner_len, top, 2.0, col);
                                overlay.line(left + w, top, left + w, top + corner_len, 2.0, col);
                                overlay.line(left + w + 1.0, top + 1.0, left + w - corner_len, top + 1.0, 1.0, shadow);
                                overlay.line(left + w + 1.0, top + 1.0, left + w + 1.0, top + corner_len, 1.0, shadow);

                                // Bottom-Left
                                overlay.line(left, top + h, left + corner_len, top + h, 2.0, col);
                                overlay.line(left, top + h, left, top + h - corner_len, 2.0, col);
                                overlay.line(left + 1.0, top + h + 1.0, left + corner_len, top + h + 1.0, 1.0, shadow);
                                overlay.line(left + 1.0, top + h + 1.0, left + 1.0, top + h - corner_len, 1.0, shadow);

                                // Bottom-Right
                                overlay.line(left + w, top + h, left + w - corner_len, top + h, 2.0, col);
                                overlay.line(left + w, top + h, left + w, top + h - corner_len, 2.0, col);
                                overlay.line(left + w + 1.0, top + h + 1.0, left + w - corner_len, top + h + 1.0, 1.0, shadow);
                                overlay.line(left + w + 1.0, top + h + 1.0, left + w + 1.0, top + h - corner_len, 1.0, shadow);
                            }
                            "Filled Box" => {
                                overlay.rect_filled(left, top, w, h, glow_col);
                                overlay.rect(left, top, w, h, col);
                            }
                            _ => {} // Off
                        }

                        // Д. Теги и надписи (Name, Confidence, Distance)
                        let scale_x = (w / 110.0).clamp(0.6, 2.0);
                        let scale_y = (h / 190.0).clamp(0.6, 2.0);

                        for tag in &cfg.tag_offsets {
                            let tx = center_x + tag.x * scale_x;
                            let ty = center_y + tag.y * scale_y;

                            match tag.id.as_str() {
                                "name" if show_name => {
                                    let label = if is_enemy { "ENEMY" } else { "TEAM" };
                                    overlay.text(tx - 18.0, ty - 6.0, label, 11.5, col);
                                }
                                "conf" if show_conf => {
                                    let label = format!("{:.0}%", target.conf * 100.0);
                                    overlay.text(tx - 12.0, ty - 6.0, &label, 10.5, Color::rgb(255, 215, 80));
                                }
                                "dist" if show_dist => {
                                    let dist_m = ((1400.0 / h.max(12.0)) * 2.8) as u32;
                                    let label = format!("{}m", dist_m);
                                    overlay.text(tx - 10.0, ty - 6.0, &label, 10.5, Color::rgb(200, 220, 245));
                                }
                                _ => {}
                            }
                        }
                    }

                    // 3. HUD в углу экрана
                    if cfg.hud_enabled {
                        let mut line_count = 1; // Title
                        if cfg.hud_show_fps { line_count += 1; }
                        if cfg.hud_show_capture_latency { line_count += 1; }
                        if cfg.hud_show_inference_latency { line_count += 1; }
                        if cfg.hud_show_render_latency { line_count += 1; }
                        if cfg.hud_show_total_latency { line_count += 1; }
                        line_count += 1; // Targets

                        let hud_w = 185.0;
                        let line_h = cfg.hud_font_size + 4.0;
                        let hud_h = 16.0 + (line_count as f32 * line_h);

                        let (hx, hy) = match cfg.hud_position.as_str() {
                            "Top-Right" => (screen_w - hud_w - 20.0, 20.0),
                            "Bottom-Left" => (20.0, screen_h - hud_h - 20.0),
                            "Bottom-Right" => (screen_w - hud_w - 20.0, screen_h - hud_h - 20.0),
                            _ => (20.0, 20.0),
                        };

                        overlay.rect_filled(hx, hy, hud_w, hud_h, Color::rgba(10, 14, 22, 220));
                        overlay.rect(hx, hy, hud_w, hud_h, Color::rgb(0, 168, 255));
                        overlay.rect_filled(hx, hy, hud_w, 2.0, Color::rgb(0, 210, 255));

                        overlay.text(hx + 10.0, hy + 8.0, "NEVERLOSE // AI HUD", cfg.hud_font_size, Color::rgb(0, 210, 255));

                        let mut y = hy + cfg.hud_font_size + 8.0;
                        if cfg.hud_show_fps {
                            overlay.text(hx + 10.0, y, &format!("FPS: {:.0}", packet.fps), cfg.hud_font_size - 1.0, Color::rgb(46, 213, 115));
                            y += line_h;
                        }

                        if cfg.hud_show_capture_latency {
                            overlay.text(hx + 10.0, y, &format!("Cap: {:.2} ms", packet.cap_ms), cfg.hud_font_size - 1.0, Color::rgb(0, 220, 255));
                            y += line_h;
                        }

                        if cfg.hud_show_inference_latency {
                            overlay.text(hx + 10.0, y, &format!("Infer: {:.1} ms", packet.infer_ms), cfg.hud_font_size - 1.0, Color::rgb(255, 71, 87));
                            y += line_h;
                        }

                        if cfg.hud_show_render_latency {
                            overlay.text(hx + 10.0, y, &format!("Render: {:.2} ms", last_render_ms), cfg.hud_font_size - 1.0, Color::rgb(165, 94, 234));
                            y += line_h;
                        }

                        if cfg.hud_show_total_latency {
                            let total_ms = packet.cap_ms + packet.infer_ms + last_render_ms;
                            overlay.text(hx + 10.0, y, &format!("Total: {:.1} ms", total_ms), cfg.hud_font_size - 1.0, Color::rgb(255, 195, 18));
                            y += line_h;
                        }

                        overlay.text(hx + 10.0, y, &format!("Targets: {}", packet.targets.len()), cfg.hud_font_size - 1.0, Color::rgb(220, 225, 235));
                    }

                    let _ = overlay.end_frame();
                    last_render_ms = (frame_render_start.elapsed().as_secs_f32() * 1000.0).clamp(0.05, 50.0);
                    if let Ok(mut lock) = live_detection.lock() {
                        lock.render_ms = last_render_ms;
                    }
                }

                // Пейсинг частоты отрисовки оверлея
                std::thread::sleep(Duration::from_millis(1));
            }
        });

        Self {
            is_running,
            thread_handle: Some(thread_handle),
        }
    }

    pub fn stop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for D3dOverlayRunner {
    fn drop(&mut self) {
        self.stop();
    }
}
