//! One frame: the background, the header, the current page, the footer, then
//! whatever is open on top (language list, dialog, dropdown).

use super::hit::{Drag, Hit};
use super::layout::{HEADER_H, MARGIN};
use super::ui::Ui;
use super::{apps, dialogs, eq, flow, footer, fx, menu, mix, outputs, rail, vst};
use super::{Drawer, Overlay, Page, Panel};
use crate::draw::{self, ButtonKind, Fonts, Rect, BG, BUTTON, TEXT, TITLE};
use crate::i18n;
use crate::worker::{CheckPhase, Snapshot};
use cairo::Context;
use std::collections::HashMap;
use std::time::Duration;

/// Icon buttons and text buttons of the header share this height.
const HEADER_BUTTON: f64 = 48.0;
const HEADER_GAP: f64 = 12.0;
const HEADER_GLYPH: f64 = 22.0;

impl Panel {
    /// The frame as RGBA bytes, for the overlay texture.
    pub fn render(&mut self, fonts: &Fonts, snap: &Snapshot, peaks: &HashMap<String, f32>) -> Vec<u8> {
        let mut surface = self.render_surface(fonts, snap, peaks);
        draw::surface_to_rgba(&mut surface)
    }

    pub fn render_surface(&mut self, fonts: &Fonts, snap: &Snapshot, peaks: &HashMap<String, f32>) -> cairo::ImageSurface {
        let (w, h) = self.pixel_size();
        let surface = draw::surface_wh(w, h).expect("surface");
        let cr = Context::new(&surface).expect("context");
        self.draw(&cr, fonts, snap, peaks);
        surface
    }

    pub fn thumbnail(&self, _fonts: &Fonts) -> Vec<u8> {
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 256, 256).expect("thumb");
        {
            let cr = Context::new(&surface).expect("context");
            let bytes = crate::brand::icon_png(self.config.accent == "pink", 256);
            draw::paint_png_bytes(&cr, bytes, Rect::square(0.0, 0.0, 256.0));
        }
        draw::surface_to_rgba(&mut surface)
    }

    fn draw(&mut self, cr: &Context, fonts: &Fonts, snap: &Snapshot, peaks: &HashMap<String, f32>) {
        self.take_snapshot(snap);
        draw::set_accent(&self.config.accent);
        self.scrollbar = None;
        self.strip = None;
        self.out_list = None;
        self.flow.canvas = None;
        // The hit lists are rebuilt every frame. Taking them keeps their allocation.
        let mut hots = std::mem::take(&mut self.hots);
        let mut faders = std::mem::take(&mut self.faders);
        hots.clear();
        faders.clear();
        let mut ui = Ui { cr, fonts, t: i18n::text(self.lang), pointer: self.pointer, pressed: self.pressed, bounds: None, hots, faders };

        ui.fill(self.window(), 28.0, BG);
        self.header(&mut ui);
        self.page(&mut ui, snap, peaks);
        footer::draw(self, &mut ui, snap.autostart);
        if self.language_open {
            footer::language_menu(self, &mut ui);
        }
        self.dialog(&mut ui, snap);
        menu::draw(self, &mut ui);

        self.hots = ui.hots;
        self.faders = ui.faders;
    }

    /// What the frame needs from the worker's snapshot before anything is drawn.
    fn take_snapshot(&mut self, snap: &Snapshot) {
        self.note_check(snap);
        if self.link != snap.link {
            self.link = snap.link.clone();
        }
        if self.vst_plugins != snap.vsts {
            self.vst_plugins = snap.vsts.clone();
        }
        if self.vst_status != snap.vst_status {
            self.vst_status = snap.vst_status.clone();
        }
    }

    /// Clears a pending Record once the worker reports it, or once it has failed.
    fn note_check(&mut self, snap: &Snapshot) {
        let failed = self.record_pending
            && snap.check != CheckPhase::Recording
            && (snap.check_epoch != self.record_epoch || self.record_at.is_some_and(|start| start.elapsed() > Duration::from_secs(8)));
        if snap.check == CheckPhase::Recording || failed {
            self.record_pending = false;
            self.record_at = None;
        }
        self.seen_epoch = snap.check_epoch;
    }

    fn page(&mut self, ui: &mut Ui, snap: &Snapshot, peaks: &HashMap<String, f32>) {
        match self.page {
            Page::Apps => apps::draw(self, ui, snap),
            Page::Flow => flow::draw(self, ui, snap, peaks),
            Page::Mix => {
                mix::draw(self, ui, snap, peaks);
                match self.drawer {
                    Drawer::Outputs => outputs::draw(self, ui, snap, peaks),
                    Drawer::FxA => fx::draw(self, ui, 0),
                    Drawer::FxB => fx::draw(self, ui, 1),
                    Drawer::Closed => {}
                }
                self.split_handle(ui);
                rail::draw(self, ui);
            }
        }
    }

    /// The open dialog, if any.
    fn dialog(&mut self, ui: &mut Ui, snap: &Snapshot) {
        match &self.overlay {
            Overlay::None => {}
            Overlay::Add => dialogs::add(self, ui, snap),
            Overlay::Mic => dialogs::mic(self, ui, snap),
            Overlay::Settings => dialogs::settings(self, ui),
            Overlay::AppsList => apps::list(self, ui, snap),
            Overlay::Confirm(key) => {
                let key = key.clone();
                apps::confirm(self, ui, snap, &key);
            }
            Overlay::Name(_) => dialogs::name(self, ui),
            Overlay::Eq(slot) => {
                let slot = *slot;
                eq::sheet(self, ui, slot);
            }
            Overlay::Projects => dialogs::projects(self, ui),
            Overlay::FxPicker(bus) => {
                let bus = *bus;
                vst::picker(self, ui, bus);
            }
            Overlay::Vst => vst::settings(self, ui),
        }
    }

    /// Logo and title on the left. On the right the Mix page offers Expand, Projects,
    /// Signal Flow and Apps; every other page offers the way back.
    fn header(&mut self, ui: &mut Ui) {
        let t = ui.t;
        let mid = MARGIN + HEADER_H / 2.0;
        let top = mid - HEADER_BUTTON / 2.0;
        let logo = crate::brand::icon_png(self.config.accent == "pink", 128);
        draw::paint_png_bytes(ui.cr, logo, Rect::square(MARGIN, mid - 18.0, 36.0));
        ui.label_mid(TITLE, TEXT, MARGIN + 50.0, mid, t.title);
        let (w, _) = self.size_f();
        if self.page != Page::Mix {
            let back = Rect::new(w - MARGIN - 200.0, top, 200.0, HEADER_BUTTON);
            ui.button_hot(back, &format!("‹  {}", t.back_mixer), ButtonKind::Plain, Hit::Back);
            return;
        }
        let apps_w = (ui.text_width(BUTTON, t.apps) + 48.0).max(150.0);
        let flow_w = (ui.text_width(BUTTON, t.flow) + 76.0).max(150.0);
        let apps = Rect::new(w - MARGIN - apps_w, top, apps_w, HEADER_BUTTON);
        let flow = Rect::new(apps.x - HEADER_GAP - flow_w, top, flow_w, HEADER_BUTTON);
        // Projects: an icon button; the dialog carries the words.
        let projects = Rect::square(flow.x - HEADER_GAP - HEADER_BUTTON, top, HEADER_BUTTON);
        header_icon(ui, projects, draw::FOLDER_PATH, 24.0, Hit::Projects);
        // Expand or reduce the whole window. The icon shows what a press does.
        let wide = Rect::square(projects.x - HEADER_GAP - HEADER_BUTTON, top, HEADER_BUTTON);
        let (glyph, hit) = if self.config.wide { (draw::MINIMIZE_PATH, Hit::Wide(false)) } else { (draw::MAXIMIZE_PATH, Hit::Wide(true)) };
        header_icon(ui, wide, glyph, 16.0, hit);
        ui.button_hot(apps, t.apps, ButtonKind::Plain, Hit::Page(Page::Apps));
        // The caption sits a little right of center, making room for the glyph.
        ui.button(flow, "", ButtonKind::Plain);
        let caption_w = ui.text_width(BUTTON, t.flow);
        let content_w = HEADER_GLYPH + 10.0 + caption_w;
        let start = flow.x + (flow.w - content_w) / 2.0;
        ui.icon(draw::FLOW_PATH, 24.0, start, mid - 11.0, HEADER_GLYPH, draw::SOFT);
        ui.label_mid(BUTTON, TEXT, start + 32.0, mid, t.flow);
        ui.hot(Hit::Page(Page::Flow), flow);
    }

    /// Grip in the gutter between the mixer and the drawer. The hit is wider than the pill.
    fn split_handle(&mut self, ui: &mut Ui) {
        let stage = self.stage();
        if stage.drawer.w < 8.0 {
            return;
        }
        let gap_x = stage.mix.right();
        let gap_w = stage.drawer.x - gap_x;
        if gap_w < 4.0 {
            return;
        }
        let cx = gap_x + gap_w / 2.0;
        let hit_w = gap_w + 12.0;
        let hit = Rect::new(cx - hit_w / 2.0, stage.mix.y, hit_w, stage.mix.h);
        let dragging = self.drag == Some(Drag::Split);
        let hover = dragging || self.pointer.is_some_and(|(px, py)| hit.contains(px, py));
        let (grip_w, grip_h) = (6.0, 56.0);
        let grip = Rect::new(cx - grip_w / 2.0, hit.y + (hit.h - grip_h) / 2.0, grip_w, grip_h);
        ui.fill(grip, 3.0, if hover { draw::accent() } else { draw::MUTED });
        ui.hot(Hit::Split, hit);
    }
}

/// A square header button with one glyph.
fn header_icon(ui: &mut Ui, rect: Rect, path: &str, view: f64, hit: Hit) {
    ui.button(rect, "", ButtonKind::Plain);
    ui.icon(path, view, rect.x + 13.0, rect.y + 13.0, HEADER_GLYPH, draw::SOFT);
    ui.hot(hit, rect);
}
