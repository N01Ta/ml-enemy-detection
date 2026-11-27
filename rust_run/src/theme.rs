use egui::{Color32, Rounding, Stroke, Visuals};

pub fn apply_custom_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    // Палитра Neverlose 1:1 по скриншоту
    let bg_dark = Color32::from_rgb(11, 13, 19);          // #0b0d13 Основной фон
    let card_bg = Color32::from_rgb(17, 21, 32);          // #111520 Фон карточек настроек
    let inner_input_bg = Color32::from_rgb(11, 14, 21);   // #0b0e15 Фон полей ввода
    let border_color = Color32::from_rgb(26, 33, 50);     // #1a2132 Тонкие границы
    let accent_cyan = Color32::from_rgb(0, 168, 255);     // #00a8ff Неоновый акцент
    let text_white = Color32::from_rgb(240, 244, 252);

    let mut visuals = Visuals::dark();
    visuals.panel_fill = bg_dark;
    visuals.window_fill = bg_dark;
    visuals.faint_bg_color = card_bg;
    visuals.extreme_bg_color = inner_input_bg;

    // Скругления как в Neverlose
    visuals.window_rounding = Rounding::same(12.0);
    visuals.menu_rounding = Rounding::same(6.0);

    // Стили виджетов
    visuals.widgets.inactive.bg_fill = inner_input_bg;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, border_color);
    visuals.widgets.inactive.rounding = Rounding::same(6.0);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, text_white);

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(22, 28, 44);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, accent_cyan);
    visuals.widgets.hovered.rounding = Rounding::same(6.0);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);

    visuals.widgets.active.bg_fill = Color32::from_rgb(0, 120, 215);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, accent_cyan);
    visuals.widgets.active.rounding = Rounding::same(6.0);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);

    visuals.selection.bg_fill = accent_cyan;
    visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);

    style.visuals = visuals;
    ctx.set_style(style);
}

pub const COLOR_BG: Color32 = Color32::from_rgb(11, 13, 19);
pub const COLOR_PANEL: Color32 = Color32::from_rgb(14, 17, 26);
pub const COLOR_CARD: Color32 = Color32::from_rgb(17, 21, 32);
pub const COLOR_BORDER: Color32 = Color32::from_rgb(26, 33, 50);
pub const COLOR_ACCENT: Color32 = Color32::from_rgb(0, 168, 255);
pub const COLOR_ACCENT_GLOW: Color32 = Color32::from_rgb(0, 230, 255);
pub const COLOR_TEXT_MUTED: Color32 = Color32::from_rgb(95, 108, 136);
