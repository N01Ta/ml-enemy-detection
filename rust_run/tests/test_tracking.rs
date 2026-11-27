use std::time::Instant;

#[derive(Default, Clone)]
struct TargetBox {
    cls: usize,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    conf: f32,
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

#[test]
fn test_tracked_targets_matching_no_panic() {
    let mut tracked_targets: Vec<TrackedTarget> = Vec::new();

    // Симулируем несколько кадров со случайным или меняющимся количеством детекций
    let frames = vec![
        // Кадр 1: 0 целей
        vec![],
        // Кадр 2: 1 цель
        vec![TargetBox { cls: 0, x: 500.0, y: 500.0, w: 60.0, h: 120.0, conf: 0.9 }],
        // Кадр 3: 3 цели (1 старая чуть сдвинулась, 2 новые) -> именно это вызвало panic у пользователя!
        vec![
            TargetBox { cls: 0, x: 505.0, y: 502.0, w: 60.0, h: 120.0, conf: 0.92 },
            TargetBox { cls: 0, x: 200.0, y: 300.0, w: 50.0, h: 100.0, conf: 0.85 },
            TargetBox { cls: 1, x: 800.0, y: 400.0, w: 70.0, h: 140.0, conf: 0.88 },
        ],
        // Кадр 4: 5 целей
        vec![
            TargetBox { cls: 0, x: 510.0, y: 505.0, w: 60.0, h: 120.0, conf: 0.91 },
            TargetBox { cls: 0, x: 205.0, y: 302.0, w: 50.0, h: 100.0, conf: 0.87 },
            TargetBox { cls: 1, x: 805.0, y: 402.0, w: 70.0, h: 140.0, conf: 0.90 },
            TargetBox { cls: 0, x: 1000.0, y: 600.0, w: 80.0, h: 160.0, conf: 0.75 },
            TargetBox { cls: 0, x: 1200.0, y: 700.0, w: 90.0, h: 180.0, conf: 0.80 },
        ],
        // Кадр 5: ушли некоторые цели
        vec![
            TargetBox { cls: 0, x: 515.0, y: 508.0, w: 60.0, h: 120.0, conf: 0.93 },
        ],
    ];

    let dt = 0.016; // ~60 FPS

    for (frame_idx, targets) in frames.into_iter().enumerate() {
        let num_existing = tracked_targets.len();
        let mut matched_indices = vec![false; num_existing];

        for raw in &targets {
            let mut best_match: Option<(usize, f32)> = None;
            for i in 0..num_existing {
                let tracked = &tracked_targets[i];
                if !matched_indices[i] && tracked.cls == raw.cls {
                    let dx = raw.x - tracked.x;
                    let dy = raw.y - tracked.y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist < 110.0 {
                        if best_match.map_or(true, |(_, d)| dist < d) {
                            best_match = Some((i, dist));
                        }
                    }
                }
            }

            if let Some((idx, _)) = best_match {
                matched_indices[idx] = true;
                let t = &mut tracked_targets[idx];
                let raw_vx = (raw.x - t.x) / dt;
                let raw_vy = (raw.y - t.y) / dt;

                t.vx = t.vx * 0.65 + raw_vx * 0.35;
                t.vy = t.vy * 0.65 + raw_vy * 0.35;

                t.x += (raw.x - t.x) * 0.35;
                t.y += (raw.y - t.y) * 0.35;
                t.w += (raw.w - t.w) * 0.35;
                t.h += (raw.h - t.h) * 0.35;
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

        println!("Frame {}: tracked_targets count = {}", frame_idx, tracked_targets.len());
    }
}
