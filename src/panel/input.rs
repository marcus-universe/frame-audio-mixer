//! Pointer events: presses, drags, the scroll sticks, and faders.
//!
//! A press is resolved against the hit areas the last frame recorded. A fader or
//! a grip starts a [`Drag`], which follows the pointer until the button is released.

use super::hit::{BusId, Drag, FaderId, Hit, Pick};
use super::ui::Ui;
use super::{eq, fx, Action, Overlay, Page, Panel};
use crate::config::{self, EffectKind};
use crate::overlay::{is_left, MOUSE_DOWN, MOUSE_MOVE, MOUSE_UP, SCROLL_DISCRETE, SCROLL_SMOOTH};
use std::time::{Duration, Instant};

/// How close, in pixels, a fader drag has to come to a snap point.
const SNAP_REACH_PX: f64 = 8.0;
/// The levels a fader snaps to, in dB.
const SNAP_MARKS: [f64; 7] = [12.0, 6.0, 0.0, -6.0, -12.0, -24.0, -48.0];
const LEVELS_EVERY: Duration = Duration::from_millis(40);
/// The scrollbar is grabbed this far outside its area too.
const SCROLLBAR_REACH: f64 = 10.0;

/// Pixels for one event of a scroll stick. A controller notch is about ±1,
/// which would otherwise move a list by one pixel.
fn scroll_step(delta: f64, notch: f64, smooth: f64) -> f64 {
    if delta.abs() <= 3.0 { delta * notch } else { delta * smooth }
}

/// The stronger of the two stick axes. On a tie the vertical one wins.
fn dominant(x: f64, y: f64) -> f64 {
    if y.abs() >= x.abs() { y } else { x }
}

impl Panel {
    pub fn pointer(&mut self, kind: u32, x: f64, y: f64, button: u32) -> Action {
        match kind {
            MOUSE_MOVE => self.moved(x, y),
            MOUSE_UP => self.released(),
            SCROLL_DISCRETE | SCROLL_SMOOTH => self.scrolled(x, y),
            MOUSE_DOWN if is_left(button) => self.pressed_at(x, y),
            _ => Action::None,
        }
    }

    fn moved(&mut self, x: f64, y: f64) -> Action {
        self.pointer = Some((x, y));
        let Some(drag) = self.drag else { return Action::None };
        match drag {
            Drag::Scrollbar => self.set_scrollbar(x, y),
            Drag::PanChannels => {
                let (dx, dy) = self.pan_delta(x, y);
                let delta = if self.config.input_list { dy } else { dx };
                self.scroll = (self.scroll - delta).max(0.0);
            }
            Drag::PanOutputs => {
                let (dx, dy) = self.pan_delta(x, y);
                let delta = if self.config.list { dx } else { dy };
                self.out_scroll = (self.out_scroll - delta).max(0.0);
            }
            Drag::PanFlow => {
                let (dx, dy) = self.pan_delta(x, y);
                self.flow.pan.0 -= dx;
                self.flow.pan.1 -= dy;
            }
            Drag::Split => self.set_split(x),
            Drag::FxGrip => self.fx_reorder(y),
            Drag::EqBand(index) => {
                if let Overlay::Eq(slot) = self.overlay {
                    eq::drag_handle(self, slot, index, x, y);
                }
                return self.live_levels();
            }
            Drag::Fader(id) => {
                self.set_fader(id, x, y);
                return self.live_levels();
            }
        }
        Action::None
    }

    /// How far the pointer moved since the last pan event. Zero on the first one.
    fn pan_delta(&mut self, x: f64, y: f64) -> (f64, f64) {
        let delta = self.pan_at.map_or((0.0, 0.0), |(px, py)| (x - px, y - py));
        self.pan_at = Some((x, y));
        delta
    }

    fn released(&mut self) -> Action {
        self.pressed = false;
        self.pan_at = None;
        self.fx_drag = None;
        match self.drag.take() {
            Some(drag) if drag.saves_on_release() => Action::Save,
            _ => Action::None,
        }
    }

    /// List rows and dialogs move on the vertical stick. The channel row moves on the horizontal one.
    fn scrolled(&mut self, x: f64, y: f64) -> Action {
        if let Overlay::Eq(slot) = self.overlay {
            let delta = if y.abs() > x.abs() { y } else { x };
            eq::nudge_q(self, slot, delta);
            return Action::Levels;
        }
        if self.overlay.scrolls() {
            self.dialog_scroll = (self.dialog_scroll - scroll_step(dominant(x, y), 150.0, 4.0) * 0.45).max(0.0);
        } else if self.page == Page::Flow {
            // Both sticks pan the flow canvas; the draw pass clamps.
            self.flow.pan.0 -= scroll_step(x, 60.0, 2.0);
            self.flow.pan.1 -= scroll_step(y, 60.0, 2.0);
        } else if self.over_outputs() {
            self.out_scroll = (self.out_scroll - scroll_step(dominant(x, y), 150.0, 4.0) * 0.45).max(0.0);
        } else if self.config.input_list {
            self.scroll = (self.scroll - scroll_step(dominant(x, y), 150.0, 4.0)).max(0.0);
        } else {
            let delta = if y.abs() > x.abs() { y } else { x };
            self.scroll = (self.scroll - scroll_step(delta, 150.0, 4.0)).max(0.0);
        }
        Action::None
    }

    fn over_outputs(&self) -> bool {
        let Some(list) = self.out_list else { return false };
        self.pointer.is_some_and(|(x, y)| list.contains(x, y))
    }

    fn pressed_at(&mut self, x: f64, y: f64) -> Action {
        self.pointer = Some((x, y));
        self.pressed = true;
        if self.menu.is_some() {
            return self.click(x, y);
        }
        if self.language_open {
            let action = self.click(x, y);
            if matches!(action, Action::None) {
                self.language_open = false;
            }
            return action;
        }
        if !matches!(self.overlay, Overlay::None) {
            return self.click(x, y);
        }
        if self.check_open || self.play_open {
            // A press anywhere but on the open selector or its list closes it.
            let open = if self.check_open { Pick::Record } else { Pick::Play };
            let on_menu = self.hit_at(x, y).is_some_and(|hit| match hit {
                Hit::PickMenu(pick) | Hit::PickMenuSurface(pick) | Hit::PickBus(pick, _) => *pick == open,
                _ => false,
            });
            if !on_menu {
                self.check_open = false;
                self.play_open = false;
                return Action::None;
            }
            return self.click(x, y);
        }
        if self.scrollbar.is_some_and(|bar| bar.rect.inset(-SCROLLBAR_REACH).contains(x, y)) {
            self.drag = Some(Drag::Scrollbar);
            self.set_scrollbar(x, y);
            return Action::None;
        }
        if let Some(id) = self.fader_at(x, y) {
            self.drag = Some(Drag::Fader(id));
            self.set_fader(id, x, y);
            self.levels_sent = None;
            return self.live_levels();
        }
        if self.hit_at(x, y).is_some() {
            return self.click(x, y);
        }
        // Empty space of a scrolling area drags it.
        let pan = if self.strip.is_some_and(|strip| strip.contains(x, y)) {
            Some(Drag::PanChannels)
        } else if self.out_list.is_some_and(|list| list.contains(x, y)) {
            Some(Drag::PanOutputs)
        } else if self.page == Page::Flow && self.flow.canvas.is_some_and(|canvas| canvas.contains(x, y)) {
            Some(Drag::PanFlow)
        } else {
            None
        };
        if pan.is_some() {
            self.drag = pan;
            self.pan_at = Some((x, y));
        }
        Action::None
    }

    fn hit_at(&self, x: f64, y: f64) -> Option<&Hit> {
        Ui::hit_at(&self.hots, x, y)
    }

    /// Presses whatever lies under the point.
    fn click(&mut self, x: f64, y: f64) -> Action {
        match self.hit_at(x, y).cloned() {
            Some(hit) => self.handle(hit),
            None => Action::None,
        }
    }

    /// Volumes while a fader is held. At most one every `LEVELS_EVERY`, so a drag does not
    /// queue a `wpctl` per pointer event. Release sends `Save` with the final value.
    pub(super) fn live_levels(&mut self) -> Action {
        let due = self.levels_sent.is_none_or(|at| at.elapsed() >= LEVELS_EVERY);
        if !due {
            return Action::None;
        }
        self.levels_sent = Some(Instant::now());
        Action::Levels
    }

    fn fader_at(&self, x: f64, y: f64) -> Option<FaderId> {
        self.faders.iter().rev().find(|fader| fader.rect.contains(x, y)).map(|fader| fader.id)
    }

    /// Moves the fader `id` to the pointer.
    fn set_fader(&mut self, id: FaderId, x: f64, y: f64) {
        let Some(area) = self.faders.iter().find(|fader| fader.id == id).copied() else { return };
        let rect = area.rect;
        let (span, along) = if area.horizontal { (rect.w, x - rect.x) } else { (rect.h, rect.y + rect.h - y) };
        let t = if span <= 0.0 { 0.0 } else { (along / span).clamp(0.0, 1.0) };
        match id {
            FaderId::FxParam { bus, index, param } => self.set_fx_param(bus, index, param, t),
            FaderId::Eq(slider) => {
                if let Overlay::Eq(slot) = self.overlay {
                    eq::set_slider(self, slot, slider, t);
                }
            }
            FaderId::Mic | FaderId::Channel(_) | FaderId::Bus(_) | FaderId::FxLevel(_) => {
                // The snap button sits on Outputs and moves only those faders. Input strips always snap.
                let magnetic = if matches!(id, FaderId::Bus(_)) { self.config.snap } else { true };
                let db = snapped_db(config::t_to_db(t), span, magnetic);
                if let Some(gain) = self.gain_mut(id) {
                    *gain = db;
                }
            }
        }
    }

    /// The gain a volume fader controls.
    fn gain_mut(&mut self, id: FaderId) -> Option<&mut f64> {
        match id {
            FaderId::Mic => Some(&mut self.config.mic_gain_db),
            FaderId::Channel(index) => self.config.channels.get_mut(index).map(|channel| &mut channel.gain_db),
            FaderId::Bus(BusId::Pc) => Some(&mut self.config.pc.gain_db),
            FaderId::Bus(BusId::A(index)) => self.config.a.get_mut(index).map(|bus| &mut bus.gain_db),
            FaderId::Bus(BusId::B(index)) => self.config.b.get_mut(index).map(|bus| &mut bus.gain_db),
            FaderId::FxLevel(bus) => Some(&mut fx::bus_mut(&mut self.config, bus).gain_db),
            FaderId::FxParam { .. } | FaderId::Eq(_) => None,
        }
    }

    fn set_fx_param(&mut self, bus: usize, index: usize, param: usize, t: f64) {
        let Some(slot) = fx::bus_mut(&mut self.config, bus).chain.get_mut(index) else { return };
        if param < slot.params.len() {
            let snap = slot.kind == EffectKind::Pitcher && param == 0 && slot.pitch_snap;
            slot.params[param] = if snap { config::pitch_snapped(t) } else { t };
        }
    }

    /// Moves the dragged effect to the chain row under the pointer.
    fn fx_reorder(&mut self, y: f64) {
        let Some(dragged) = self.fx_drag else { return };
        let top = self.stage().drawer.y + fx::CHAIN_TOP;
        let chain = &mut fx::bus_mut(&mut self.config, dragged.bus).chain;
        if chain.len() < 2 || dragged.index >= chain.len() {
            return;
        }
        let index = ((y - top) / fx::CHAIN_STEP).floor().clamp(0.0, (chain.len() - 1) as f64) as usize;
        if index == dragged.index {
            return;
        }
        let slot = chain.remove(dragged.index);
        chain.insert(index.min(chain.len()), slot);
        let index = index.min(chain.len().saturating_sub(1));
        self.fx_drag = Some(super::FxSlot { bus: dragged.bus, index });
    }

    /// The thumb follows the pointer; a click on the track jumps there.
    fn set_scrollbar(&mut self, x: f64, y: f64) {
        let Some(bar) = self.scrollbar else { return };
        let (pos, origin, span) = if bar.vertical { (y, bar.rect.y, bar.rect.h) } else { (x, bar.rect.x, bar.rect.w) };
        let travel = span - bar.thumb_len;
        if travel <= 0.0 {
            return;
        }
        let t = ((pos - origin - bar.thumb_len / 2.0) / travel).clamp(0.0, 1.0);
        self.scroll = t * bar.max_scroll;
    }
}

/// `db`, pulled onto a snap mark when the pointer is within reach of it.
/// `span` is the fader's travel in pixels.
fn snapped_db(db: f64, span: f64, magnetic: bool) -> f64 {
    if !magnetic || !db.is_finite() {
        return db;
    }
    let reach = if span > 0.0 { SNAP_REACH_PX / span * (config::FADER_DB_MAX - config::FADER_DB_MIN) } else { 0.8 };
    SNAP_MARKS.into_iter().find(|mark| (db - mark).abs() <= reach.max(0.8)).unwrap_or(db)
}
