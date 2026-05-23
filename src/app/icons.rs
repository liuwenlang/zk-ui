use eframe::egui;

use super::types::ZnodeIconKind;
use super::ZkApp;

impl ZkApp {
    pub(crate) fn icon_slot_size() -> f32 {
        16.0
    }

    /// Visual center for icons — nudge down to match text cap-height.
    pub(crate) fn icon_draw_center(rect: egui::Rect) -> egui::Pos2 {
        rect.center() + egui::vec2(0.0, 1.0)
    }

    pub(crate) fn icon_rect_clickable(ui: &mut egui::Ui) -> (egui::Rect, egui::Pos2, egui::Response) {
        let s = Self::icon_slot_size();
        let size = egui::vec2(s, s);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
        let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        (rect, rect.center(), response)
    }

    /// Resource-manager folder (JetBrains-style): closed / open by expand state.
    pub(crate) fn paint_resource_folder_icon(ui: &mut egui::Ui, expanded: bool) -> egui::Response {
        let (rect, _, resp) = Self::icon_rect_clickable(ui);
        let c = Self::icon_draw_center(rect);
        let p = ui.painter();
        let tab_color = egui::Color32::from_rgb(95, 125, 155);
        let body_color = egui::Color32::from_rgb(115, 145, 175);
        let inner_color = egui::Color32::from_rgb(145, 170, 195);
        let stroke = egui::Stroke::new(0.5, egui::Color32::from_rgb(75, 100, 130));
        if expanded {
            let body = egui::Rect::from_min_size(egui::pos2(c.x - 6.0, c.y - 2.0), egui::vec2(12.0, 8.0));
            let tab = egui::Rect::from_min_size(egui::pos2(c.x - 5.5, c.y - 4.5), egui::vec2(7.0, 2.5));
            p.rect_filled(body, 1.5, body_color);
            p.rect_filled(
                egui::Rect::from_min_size(body.min + egui::vec2(1.5, 1.5), egui::vec2(9.0, 5.5)),
                1.0,
                inner_color,
            );
            p.rect_filled(tab, 1.0, tab_color);
            p.rect_stroke(body, 1.5, stroke);
        } else {
            let body = egui::Rect::from_min_size(egui::pos2(c.x - 5.5, c.y - 1.5), egui::vec2(11.0, 7.5));
            let tab = egui::Rect::from_min_size(egui::pos2(c.x - 5.5, c.y - 4.0), egui::vec2(6.5, 2.5));
            p.rect_filled(tab, 1.0, tab_color);
            p.rect_filled(body, 1.5, body_color);
            p.rect_stroke(body, 1.5, stroke);
        }
        resp
    }

    /// ZK znode folder — amber; open shows interior grid, closed is compact.
    pub(crate) fn paint_zk_znode_folder_icon(ui: &mut egui::Ui, expanded: bool) -> egui::Response {
        let (rect, _, resp) = Self::icon_rect_clickable(ui);
        let c = Self::icon_draw_center(rect);
        let p = ui.painter();
        let tab_color = egui::Color32::from_rgb(195, 130, 45);
        let body_color = egui::Color32::from_rgb(225, 165, 55);
        let inner_color = egui::Color32::from_rgb(245, 200, 120);
        let stroke = egui::Stroke::new(0.6, egui::Color32::from_rgb(160, 100, 30));
        let dot_color = egui::Color32::from_rgb(140, 90, 25);
        if expanded {
            let body = egui::Rect::from_min_size(egui::pos2(c.x - 5.5, c.y - 2.0), egui::vec2(11.0, 8.0));
            let tab = egui::Rect::from_min_size(egui::pos2(c.x - 5.0, c.y - 4.5), egui::vec2(6.0, 2.5));
            p.rect_filled(body, 1.5, body_color);
            p.rect_filled(
                egui::Rect::from_min_size(body.min + egui::vec2(1.5, 1.5), egui::vec2(8.0, 5.0)),
                1.0,
                inner_color,
            );
            p.rect_filled(tab, 1.0, tab_color);
            p.rect_stroke(body, 1.5, stroke);
            for (dx, dy) in [(0.0, 0.5), (3.0, 0.5), (0.0, 3.0), (3.0, 3.0)] {
                p.circle_filled(egui::pos2(c.x - 2.0 + dx, c.y + dy), 1.0, dot_color);
            }
        } else {
            let body = egui::Rect::from_min_size(egui::pos2(c.x - 5.5, c.y - 1.5), egui::vec2(11.0, 7.5));
            let tab = egui::Rect::from_min_size(egui::pos2(c.x - 5.5, c.y - 4.0), egui::vec2(6.0, 2.5));
            p.rect_filled(tab, 1.0, tab_color);
            p.rect_filled(body, 1.5, body_color);
            p.rect_stroke(body, 1.5, stroke);
        }
        resp
    }

    /// Leaf znode — document / page icon.
    pub(crate) fn paint_znode_document_icon(ui: &mut egui::Ui) {
        let s = Self::icon_slot_size();
        let (rect, _) = ui.allocate_exact_size(egui::vec2(s, s), egui::Sense::hover());
        let c = Self::icon_draw_center(rect);
        let p = ui.painter();
        let page = egui::Rect::from_min_size(egui::pos2(c.x - 4.5, c.y - 4.5), egui::vec2(8.0, 10.0));
        p.rect_filled(page, 1.0, egui::Color32::from_rgb(235, 238, 242));
        p.rect_stroke(page, 1.0, egui::Stroke::new(0.8, egui::Color32::from_rgb(150, 165, 180)));
        let fold = [
            egui::pos2(c.x + 3.5, c.y - 4.5),
            egui::pos2(c.x + 3.5, c.y - 1.5),
            egui::pos2(c.x + 0.5, c.y - 4.5),
        ];
        p.add(egui::Shape::convex_polygon(
            fold.to_vec(),
            egui::Color32::from_rgb(200, 208, 218),
            egui::Stroke::new(0.5, egui::Color32::from_rgb(150, 165, 180)),
        ));
        for dy in [-1.5, 0.5, 2.0] {
            p.hline(
                (c.x - 3.0)..=(c.x + 1.5),
                c.y + dy,
                egui::Stroke::new(0.8, egui::Color32::from_rgb(170, 185, 200)),
            );
        }
    }

    pub(crate) fn paint_znode_icon(ui: &mut egui::Ui, kind: ZnodeIconKind, expanded: bool) -> Option<egui::Response> {
        match kind {
            ZnodeIconKind::Root | ZnodeIconKind::ZkFolder => {
                Some(Self::paint_zk_znode_folder_icon(ui, expanded))
            }
            ZnodeIconKind::Document => {
                Self::paint_znode_document_icon(ui);
                None
            }
        }
    }

    /// ZooKeeper connection — clustered nodes motif.
    pub(crate) fn paint_zk_connection_icon(ui: &mut egui::Ui, connected: bool) -> egui::Response {
        let (rect, _, resp) = Self::icon_rect_clickable(ui);
        let c = Self::icon_draw_center(rect);
        let p = ui.painter();
        let main = if connected {
            egui::Color32::from_rgb(55, 175, 95)
        } else {
            egui::Color32::from_rgb(140, 145, 150)
        };
        let sub = if connected {
            egui::Color32::from_rgb(40, 140, 75)
        } else {
            egui::Color32::from_rgb(110, 115, 120)
        };
        let stroke = egui::Stroke::new(1.0, sub);
        p.circle_filled(c, 3.5, main);
        for (dx, dy) in [(-5.5, -3.0), (5.5, -3.0), (0.0, 5.0)] {
            let pt = egui::pos2(c.x + dx, c.y + dy);
            p.line_segment([c, pt], stroke);
            p.circle_filled(pt, 2.0, sub);
        }
        resp
    }
}
