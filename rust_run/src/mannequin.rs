use egui::{pos2, vec2, Color32, Painter, Pos2, Rect, Response, Sense, Stroke, Ui};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BodyPart {
    Head,
    Neck,
    Chest,
    Pelvis,
    LeftShoulder,
    RightShoulder,
    LeftArm,
    RightArm,
    LeftKnee,
    RightKnee,
    LeftFoot,
    RightFoot,
}

#[derive(Clone, Debug)]
pub struct TargetBone {
    pub part: BodyPart,
    pub name: &'static str,
    pub rel_pos: Pos2, // [0.0..1.0] относительно центра модели
    pub enabled: bool,
}

pub struct MannequinWidget {
    pub bones: Vec<TargetBone>,
    pub selected_part: Option<BodyPart>,
}

impl Default for MannequinWidget {
    fn default() -> Self {
        Self {
            bones: vec![
                TargetBone { part: BodyPart::Head, name: "Head", rel_pos: pos2(0.5, 0.13), enabled: true },
                TargetBone { part: BodyPart::Neck, name: "Neck", rel_pos: pos2(0.5, 0.20), enabled: false },
                TargetBone { part: BodyPart::Chest, name: "Chest", rel_pos: pos2(0.5, 0.28), enabled: true },
                TargetBone { part: BodyPart::Pelvis, name: "Stomach", rel_pos: pos2(0.5, 0.44), enabled: true },
                TargetBone { part: BodyPart::LeftShoulder, name: "L-Shoulder", rel_pos: pos2(0.36, 0.25), enabled: false },
                TargetBone { part: BodyPart::RightShoulder, name: "R-Shoulder", rel_pos: pos2(0.64, 0.25), enabled: false },
                TargetBone { part: BodyPart::LeftArm, name: "L-Hand", rel_pos: pos2(0.32, 0.48), enabled: false },
                TargetBone { part: BodyPart::RightArm, name: "R-Hand", rel_pos: pos2(0.68, 0.48), enabled: false },
                TargetBone { part: BodyPart::LeftKnee, name: "L-Knee", rel_pos: pos2(0.40, 0.68), enabled: false },
                TargetBone { part: BodyPart::RightKnee, name: "R-Knee", rel_pos: pos2(0.60, 0.68), enabled: false },
                TargetBone { part: BodyPart::LeftFoot, name: "L-Foot", rel_pos: pos2(0.39, 0.88), enabled: false },
                TargetBone { part: BodyPart::RightFoot, name: "R-Foot", rel_pos: pos2(0.61, 0.88), enabled: false },
            ],
            selected_part: Some(BodyPart::Head),
        }
    }
}

impl MannequinWidget {
    pub fn show(&mut self, ui: &mut Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 410.0), Sense::click());
        let painter = ui.painter_at(rect);

        // Фоновый градиентный человеческий силуэт
        self.draw_soldier_silhouette(&painter, rect);

        // Интерактивные точки (Bones)
        let mouse_pos = response.interact_pointer_pos();

        for bone in &mut self.bones {
            let screen_pos = pos2(
                rect.min.x + bone.rel_pos.x * rect.width(),
                rect.min.y + bone.rel_pos.y * rect.height(),
            );

            let dot_radius = 6.5;
            let dot_rect = Rect::from_center_size(screen_pos, vec2(dot_radius * 3.0, dot_radius * 3.0));
            let hovered = mouse_pos.map_or(false, |p| dot_rect.contains(p));

            if hovered && response.clicked() {
                bone.enabled = !bone.enabled;
                self.selected_part = Some(bone.part);
            }

            // Отрисовка светящихся узлов Neverlose
            if bone.enabled {
                // Внешнее свечение (Неоновый акцент)
                painter.circle_filled(screen_pos, dot_radius + 6.0, Color32::from_rgba_unmultiplied(0, 195, 255, 30));
                painter.circle_filled(screen_pos, dot_radius + 3.0, Color32::from_rgba_unmultiplied(0, 210, 255, 75));
                painter.circle_stroke(screen_pos, dot_radius + 1.5, Stroke::new(1.5, Color32::from_rgb(0, 235, 255)));
                
                // Внутреннее ядро
                painter.circle_filled(screen_pos, dot_radius - 1.0, Color32::from_rgb(0, 160, 255));
                painter.circle_filled(screen_pos, 2.8, Color32::WHITE);
                
                // Маленький прицельный крестик в центре как в оригинале
                let cross_len = 3.5;
                let cross_stroke = Stroke::new(1.0, Color32::from_rgb(0, 240, 255));
                painter.line_segment([pos2(screen_pos.x - cross_len, screen_pos.y), pos2(screen_pos.x + cross_len, screen_pos.y)], cross_stroke);
                painter.line_segment([pos2(screen_pos.x, screen_pos.y - cross_len), pos2(screen_pos.x, screen_pos.y + cross_len)], cross_stroke);
            } else {
                // Неактивный узел
                let fill = if hovered { Color32::from_rgb(45, 55, 80) } else { Color32::from_rgb(26, 32, 48) };
                let stroke = if hovered { Color32::from_rgb(100, 120, 160) } else { Color32::from_rgb(48, 58, 82) };
                painter.circle_filled(screen_pos, 5.0, fill);
                painter.circle_stroke(screen_pos, 5.5, Stroke::new(1.0, stroke));
                
                // Точечка в центре
                painter.circle_filled(screen_pos, 1.5, Color32::from_rgb(70, 85, 115));
            }

            // Тултип при наведении
            if hovered {
                let text_pos = pos2(screen_pos.x + 12.0, screen_pos.y - 7.0);
                painter.rect_filled(
                    Rect::from_min_size(pos2(text_pos.x - 4.0, text_pos.y - 2.0), vec2(bone.name.len() as f32 * 7.5 + 8.0, 18.0)),
                    egui::Rounding::same(4.0),
                    Color32::from_rgb(14, 18, 28)
                );
                painter.text(
                    text_pos,
                    egui::Align2::LEFT_TOP,
                    bone.name,
                    egui::FontId::proportional(11.0),
                    Color32::from_rgb(0, 230, 255),
                );
            }
        }

        response
    }

    fn draw_soldier_silhouette(&self, painter: &Painter, rect: Rect) {
        let cx = rect.center().x;
        let top = rect.min.y + 25.0;

        // Фирменный цвет скелета / силуэта из Neverlose
        let body_col = Color32::from_rgb(24, 30, 44);
        let limb_stroke = Stroke::new(2.4, Color32::from_rgb(32, 42, 62));
        let joint_stroke = Stroke::new(1.5, Color32::from_rgb(44, 58, 86));

        // 1. Голова (шлем)
        let head_center = pos2(cx, top + 28.0);
        painter.circle_filled(head_center, 18.0, body_col);
        painter.circle_stroke(head_center, 18.0, joint_stroke);
        // Забрало шлема
        painter.line_segment([pos2(cx - 9.0, head_center.y + 1.0), pos2(cx + 9.0, head_center.y + 1.0)], Stroke::new(2.0, Color32::from_rgb(50, 68, 100)));

        // 2. Торс (Трапециевидное тело бойца)
        let chest_top_w = 28.0;
        let chest_bottom_w = 20.0;
        let chest_y1 = top + 52.0;
        let chest_y2 = top + 155.0;
        
        let torso_mesh = vec![
            pos2(cx - chest_top_w, chest_y1),
            pos2(cx + chest_top_w, chest_y1),
            pos2(cx + chest_bottom_w, chest_y2),
            pos2(cx - chest_bottom_w, chest_y2),
        ];
        painter.add(egui::Shape::convex_polygon(torso_mesh, body_col, joint_stroke));

        // 3. Руки бойца
        // Левая рука (плечо -> локоть -> кисть)
        painter.line_segment([pos2(cx - chest_top_w, chest_y1 + 10.0), pos2(cx - 42.0, top + 110.0)], limb_stroke);
        painter.line_segment([pos2(cx - 42.0, top + 110.0), pos2(cx - 46.0, top + 172.0)], limb_stroke);
        
        // Правая рука
        painter.line_segment([pos2(cx + chest_top_w, chest_y1 + 10.0), pos2(cx + 42.0, top + 110.0)], limb_stroke);
        painter.line_segment([pos2(cx + 42.0, top + 110.0), pos2(cx + 46.0, top + 172.0)], limb_stroke);

        // 4. Ноги бойца
        let pelvis_y = chest_y2;
        // Левая нога (таз -> колено -> стопа)
        painter.line_segment([pos2(cx - 15.0, pelvis_y), pos2(cx - 24.0, top + 255.0)], limb_stroke);
        painter.line_segment([pos2(cx - 24.0, top + 255.0), pos2(cx - 26.0, top + 338.0)], limb_stroke);

        // Правая нога
        painter.line_segment([pos2(cx + 15.0, pelvis_y), pos2(cx + 24.0, top + 255.0)], limb_stroke);
        painter.line_segment([pos2(cx + 24.0, top + 255.0), pos2(cx + 26.0, top + 338.0)], limb_stroke);
    }
}
