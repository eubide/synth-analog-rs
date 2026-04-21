#![allow(dead_code)]

use eframe::egui::{self, Color32, Pos2, Response, Sense, Stroke, Ui};
use std::ops::RangeInclusive;
use std::f32::consts::TAU;

const KNOB_SIZE: f32 = 36.0;
const START_ANGLE: f32 = TAU * 0.625; // 225°
const SWEEP: f32 = TAU * 0.75;        // 270° de barrido
const AMBER: Color32 = Color32::from_rgb(0xe8, 0x97, 0x1a);
const TRACK_BG: Color32 = Color32::from_rgb(0x44, 0x44, 0x44);
const FACE_BG: Color32 = Color32::from_rgb(0x2a, 0x2a, 0x2a);

/// Knob circular. Devuelve la `Response` con `.changed()` si el valor cambió.
///
/// - Drag vertical: sube/baja proporcional al rango
/// - Shift+drag: ajuste fino (÷10)
/// - Doble click: reset al default (valor mínimo del rango)
/// - Hover: tooltip con valor numérico
pub fn knob(
    ui: &mut Ui,
    value: &mut f32,
    range: RangeInclusive<f32>,
    label: &str,
    default: f32,
) -> Response {
    let size = egui::vec2(KNOB_SIZE, KNOB_SIZE + 14.0); // +14 para etiqueta
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click_and_drag());

    let knob_rect = egui::Rect::from_center_size(
        egui::pos2(rect.center().x, rect.top() + KNOB_SIZE / 2.0),
        egui::vec2(KNOB_SIZE, KNOB_SIZE),
    );

    // Doble click → reset
    if response.double_clicked() {
        if *value != default {
            *value = default;
            response.mark_changed();
        }
    }

    // Drag vertical
    if response.dragged() {
        let delta = -response.drag_delta().y; // invertido: ↑ sube
        let fine = ui.input(|i| i.modifiers.shift);
        let sensitivity = if fine { 0.001 } else { 0.01 };
        let span = *range.end() - *range.start();
        let delta_v = delta * sensitivity * span;
        let new_val = (*value + delta_v).clamp(*range.start(), *range.end());
        if new_val != *value {
            *value = new_val;
            response.mark_changed();
        }
    }

    // Tooltip
    if response.hovered() {
        response = response.on_hover_text(format!("{}: {:.3}", label, value));
    }

    // Render
    if ui.is_rect_visible(knob_rect) {
        let painter = ui.painter();
        let center = knob_rect.center();
        let radius = KNOB_SIZE / 2.0 - 2.0;

        // Fondo
        painter.circle_filled(center, radius, FACE_BG);

        // Arco de rango completo (track de fondo)
        paint_arc(painter, center, radius - 3.0, 0.0, 1.0, TRACK_BG, 3.0);

        // Arco de valor (ámbar)
        let t = (*value - range.start()) / (*range.end() - range.start()).max(f32::EPSILON);
        if t > 0.0 {
            paint_arc(painter, center, radius - 3.0, 0.0, t, AMBER, 3.0);
        }

        // Tick indicador
        let angle = START_ANGLE + t * SWEEP;
        let inner = radius - 8.0;
        let outer = radius - 2.0;
        let tick_start = Pos2::new(center.x + inner * angle.cos(), center.y + inner * angle.sin());
        let tick_end = Pos2::new(center.x + outer * angle.cos(), center.y + outer * angle.sin());
        painter.line_segment([tick_start, tick_end], Stroke::new(2.0, Color32::WHITE));

        // Borde exterior sutil
        painter.circle_stroke(center, radius, Stroke::new(1.0, Color32::from_gray(60)));

        // Etiqueta debajo
        let label_pos = egui::pos2(center.x, knob_rect.bottom() + 3.0);
        painter.text(
            label_pos,
            egui::Align2::CENTER_TOP,
            label,
            egui::FontId::proportional(9.5),
            Color32::from_gray(0xaa),
        );
    }

    response
}

/// Pinta un arco usando segmentos de línea. `t_start` y `t_end` en [0,1]
/// sobre el barrido de 270° (de 225° a 315°, horario).
fn paint_arc(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    t_start: f32,
    t_end: f32,
    color: Color32,
    width: f32,
) {
    let steps = 32;
    let stroke = Stroke::new(width, color);
    let mut prev: Option<Pos2> = None;
    for i in 0..=steps {
        let t = t_start + (t_end - t_start) * (i as f32 / steps as f32);
        let angle = START_ANGLE + t * SWEEP;
        let pt = Pos2::new(center.x + radius * angle.cos(), center.y + radius * angle.sin());
        if let Some(p) = prev {
            painter.line_segment([p, pt], stroke);
        }
        prev = Some(pt);
    }
}
