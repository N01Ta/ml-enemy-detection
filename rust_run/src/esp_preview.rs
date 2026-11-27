use egui::{pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Rounding, Sense, Stroke, Ui};

#[derive(Clone, Debug)]
pub struct DraggableTag {
    pub id: &'static str,
    #[allow(dead_code)]
    pub name: &'static str,
    pub sample_text: &'static str,
    pub offset: Pos2, // Смещение относительно центра бокса (в пикселях)
    pub is_dragging: bool,
}

#[derive(Clone, Debug)]
pub struct EspPreviewWidget {
    pub preview_enemy: bool, // true = Enemy, false = Friendly
    pub tags: Vec<DraggableTag>,
}

impl Default for EspPreviewWidget {
    fn default() -> Self {
        Self {
            preview_enemy: true,
            tags: vec![
                DraggableTag {
                    id: "name",
                    name: "Name / Class",
                    sample_text: "ENEMY",
                    offset: pos2(0.0, -112.0), // Сверху бокса
                    is_dragging: false,
                },
                DraggableTag {
                    id: "conf",
                    name: "Confidence",
                    sample_text: "96.4%",
                    offset: pos2(0.0, -94.0), // Чуть ниже имени
                    is_dragging: false,
                },
                DraggableTag {
                    id: "dist",
                    name: "Distance",
                    sample_text: "32m",
                    offset: pos2(0.0, 108.0), // Снизу бокса
                    is_dragging: false,
                },
            ],
        }
    }
}

impl EspPreviewWidget {
    pub fn reset_positions(&mut self) {
        *self = Self::default();
    }

    pub fn render(
        &mut self,
        ui: &mut Ui,
        box_type: &str,
        enemy_rgb: [u8; 3],
        friendly_rgb: [u8; 3],
        head_dot: bool,
        predict: bool,
        snapline: bool,
        show_conf: bool,
        show_name: bool,
        show_dist: bool,
    ) {
        let (rect, _response) = ui.allocate_exact_size(vec2(ui.available_width(), 410.0), Sense::hover());
        let painter = ui.painter_at(rect);

        // Центр области предпросмотра
        let center = rect.center();

        // Текущий активный цвет (Враг или Союзник)
        let active_rgb = if self.preview_enemy { enemy_rgb } else { friendly_rgb };
        let active_color = Color32::from_rgb(active_rgb[0], active_rgb[1], active_rgb[2]);
        let active_glow = Color32::from_rgba_unmultiplied(active_rgb[0], active_rgb[1], active_rgb[2], 30);

        // Размеры Bounding Box бойца
        let box_w = 110.0;
        let box_h = 190.0;
        let bbox = Rect::from_center_size(center, vec2(box_w, box_h));
        let head_pos = pos2(center.x, center.y - 68.0);

        // 1. Отрисовка силуэта бойца
        self.draw_soldier_silhouette(&painter, center);

        // 2. Линия привязки (Snapline) снизу экрана
        if snapline {
            let bottom_center = pos2(center.x, rect.bottom() - 10.0);
            let snap_col = Color32::from_rgba_unmultiplied(active_rgb[0], active_rgb[1], active_rgb[2], 160);
            painter.line_segment([bottom_center, pos2(center.x, bbox.bottom())], Stroke::new(1.4, snap_col));
        }

        // 3. Предикт движения (Velocity Lead Vector + Ghost Dot)
        if predict {
            let lead_offset = vec2(36.0, -8.0);
            let pred_head = head_pos + lead_offset;
            let pred_col = Color32::from_rgb(255, 195, 18); // Gold / Neon Yellow Lead

            // Векторная стрелка упреждения
            painter.line_segment([head_pos, pred_head], Stroke::new(1.8, pred_col));
            painter.circle_filled(pred_head, 3.5, pred_col);
            painter.circle_stroke(pred_head, 6.0, Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 195, 18, 120)));
            painter.text(
                pred_head + vec2(8.0, -4.0),
                Align2::LEFT_CENTER,
                "PRED +35ms",
                FontId::proportional(9.5),
                pred_col,
            );
        }

        // 4. Точка прицеливания в голову (Head Dot & Tactical Cross)
        if head_dot {
            // Внешнее прицельное кольцо
            painter.circle_stroke(head_pos, 7.0, Stroke::new(1.2, active_color));
            // 4 тактические риски
            painter.line_segment([head_pos + vec2(-10.0, 0.0), head_pos + vec2(-5.0, 0.0)], Stroke::new(1.2, active_color));
            painter.line_segment([head_pos + vec2(5.0, 0.0), head_pos + vec2(10.0, 0.0)], Stroke::new(1.2, active_color));
            painter.line_segment([head_pos + vec2(0.0, -10.0), head_pos + vec2(0.0, -5.0)], Stroke::new(1.2, active_color));
            painter.line_segment([head_pos + vec2(0.0, 5.0), head_pos + vec2(0.0, 10.0)], Stroke::new(1.2, active_color));
            // Центральная неоновая точка с белым ядром
            painter.circle_filled(head_pos, 3.2, active_color);
            painter.circle_filled(head_pos, 1.4, Color32::WHITE);
        }

        // 5. Отрисовка Bounding Box
        match box_type {
            "2D Box" => {
                painter.rect_stroke(bbox, Rounding::same(3.0), Stroke::new(1.8, active_color));
                painter.rect_stroke(bbox.expand(1.0), Rounding::same(4.0), Stroke::new(1.0, Color32::from_black_alpha(120)));
            }
            "Corner Box" => {
                let corner_len = 18.0;
                let stroke = Stroke::new(2.2, active_color);
                let shadow_stroke = Stroke::new(1.0, Color32::from_black_alpha(160));

                let corners = [
                    (bbox.left_top(), vec2(corner_len, 0.0), vec2(0.0, corner_len)),
                    (bbox.right_top(), vec2(-corner_len, 0.0), vec2(0.0, corner_len)),
                    (bbox.left_bottom(), vec2(corner_len, 0.0), vec2(0.0, -corner_len)),
                    (bbox.right_bottom(), vec2(-corner_len, 0.0), vec2(0.0, -corner_len)),
                ];

                for (origin, h, v) in corners {
                    painter.line_segment([origin, origin + h], stroke);
                    painter.line_segment([origin, origin + v], stroke);
                    painter.line_segment([origin + vec2(1.0, 1.0), origin + h + vec2(1.0, 1.0)], shadow_stroke);
                    painter.line_segment([origin + vec2(1.0, 1.0), origin + v + vec2(1.0, 1.0)], shadow_stroke);
                }
            }
            "Filled Box" => {
                painter.rect_filled(bbox, Rounding::same(3.0), active_glow);
                painter.rect_stroke(bbox, Rounding::same(3.0), Stroke::new(1.5, active_color));
            }
            _ => {} // Off
        }

        // 6. ИНТЕРАКТИВНЫЕ ПЕРЕТАСКИВАЕМЫЕ ЭЛЕМЕНТЫ (DRAGGABLE TAGS)
        let mouse_pos = ui.input(|i| i.pointer.hover_pos());
        let pointer_down = ui.input(|i| i.pointer.primary_down());
        let pointer_released = ui.input(|i| i.pointer.primary_released());

        for tag in &mut self.tags {
            let is_visible = match tag.id {
                "name" => show_name,
                "conf" => show_conf,
                "dist" => show_dist,
                _ => true,
            };

            if !is_visible {
                continue;
            }

            let tag_center = center + vec2(tag.offset.x, tag.offset.y);
            let display_text = if tag.id == "name" {
                if self.preview_enemy { "ENEMY" } else { "TEAMMATE" }
            } else {
                tag.sample_text
            };

            let text_size = 11.0;
            let padding = vec2(10.0, 4.0);
            let approx_w = (display_text.len() as f32 * 6.5) + padding.x * 2.0;
            let approx_h = 20.0;
            let tag_rect = Rect::from_center_size(tag_center, vec2(approx_w, approx_h));

            // Логика перетаскивания (Drag & Drop)
            let is_hovered = mouse_pos.map_or(false, |p| tag_rect.contains(p));

            if is_hovered && pointer_down && !tag.is_dragging {
                tag.is_dragging = true;
            }

            if tag.is_dragging {
                if pointer_down {
                    if let Some(pos) = mouse_pos {
                        tag.offset.x = (pos.x - center.x).clamp(-140.0, 140.0);
                        tag.offset.y = (pos.y - center.y).clamp(-160.0, 160.0);
                    }
                } else if pointer_released {
                    tag.is_dragging = false;
                }
            }

            // Отрисовка плашки тега
            let bg_color = if tag.is_dragging {
                Color32::from_rgb(28, 42, 68)
            } else if is_hovered {
                Color32::from_rgb(20, 26, 42)
            } else {
                Color32::from_rgba_unmultiplied(12, 15, 24, 220)
            };

            let border_color = if tag.is_dragging {
                Color32::from_rgb(0, 220, 255)
            } else if is_hovered {
                Color32::from_rgb(0, 160, 240)
            } else {
                Color32::from_rgb(32, 42, 64)
            };

            painter.rect_filled(tag_rect, Rounding::same(4.0), bg_color);
            painter.rect_stroke(tag_rect, Rounding::same(4.0), Stroke::new(1.0, border_color));

            // Ручка перетаскивания (⋮)
            let handle_color = if tag.is_dragging { Color32::from_rgb(0, 230, 255) } else { Color32::from_rgb(80, 95, 125) };
            painter.text(
                pos2(tag_rect.left() + 4.0, tag_rect.center().y),
                Align2::LEFT_CENTER,
                "⋮",
                FontId::proportional(12.0),
                handle_color,
            );

            // Текст тега
            let text_color = if tag.id == "name" {
                active_color
            } else if tag.id == "conf" {
                Color32::from_rgb(255, 215, 80)
            } else if tag.id == "dist" {
                Color32::from_rgb(200, 220, 245)
            } else {
                Color32::WHITE
            };

            painter.text(
                pos2(tag_rect.left() + 14.0, tag_rect.center().y),
                Align2::LEFT_CENTER,
                display_text,
                FontId::proportional(text_size),
                text_color,
            );
        }
    }

    fn draw_soldier_silhouette(&self, painter: &Painter, center: Pos2) {
        let body_col = Color32::from_rgb(24, 28, 40);
        let border_col = Color32::from_rgb(38, 48, 70);

        // 1. Шлем / Голова
        let head_c = pos2(center.x, center.y - 68.0);
        painter.circle_filled(head_c, 14.0, body_col);
        painter.circle_stroke(head_c, 14.0, Stroke::new(1.5, border_col));

        // 2. Торс
        let chest_top = head_c.y + 16.0;
        let torso_pts = vec![
            pos2(center.x - 22.0, chest_top),
            pos2(center.x + 22.0, chest_top),
            pos2(center.x + 14.0, center.y + 12.0),
            pos2(center.x - 14.0, center.y + 12.0),
        ];
        painter.add(egui::Shape::convex_polygon(torso_pts, body_col, Stroke::new(1.5, border_col)));

        // 3. Руки
        painter.line_segment([pos2(center.x - 22.0, chest_top + 4.0), pos2(center.x - 36.0, center.y + 8.0)], Stroke::new(5.5, body_col));
        painter.line_segment([pos2(center.x + 22.0, chest_top + 4.0), pos2(center.x + 36.0, center.y + 8.0)], Stroke::new(5.5, body_col));

        // 4. Ноги
        let pelvis_y = center.y + 12.0;
        painter.line_segment([pos2(center.x - 10.0, pelvis_y), pos2(center.x - 18.0, center.y + 84.0)], Stroke::new(6.0, body_col));
        painter.line_segment([pos2(center.x + 10.0, pelvis_y), pos2(center.x + 18.0, center.y + 84.0)], Stroke::new(6.0, body_col));
    }
}
