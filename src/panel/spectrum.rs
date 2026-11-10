//! The analyzer behind the parametric EQ: line, filled area, bars, or a heat map
//! that scrolls upward, newest at the bottom.

use crate::config::EqView;
use crate::draw::{self, Rect};
use crate::fx::analyzer::{band_hz, BANDS};
use cairo::{Context, Extend, Filter, Format, ImageSurface, LinearGradient};

/// Four seconds at the feed's 30 updates per second.
const HEAT_ROWS: i32 = 120;
const FLOOR_DB: f32 = -78.0;
const RANGE_DB: f32 = 72.0;
/// Music falls about this many dB per octave. Tilting it back reads level evenly.
const TILT_DB_PER_OCTAVE: f32 = 3.0;
const FALL: f32 = 0.82;
const BAR_COUNT: usize = 64;

/// (position, red, green, blue). Dark blue through magenta and orange to white.
const HEAT_STOPS: [(f32, f32, f32, f32); 7] = [
    (0.0, 14.0, 16.0, 21.0),
    (0.18, 24.0, 22.0, 92.0),
    (0.4, 126.0, 28.0, 142.0),
    (0.6, 218.0, 44.0, 84.0),
    (0.76, 250.0, 128.0, 30.0),
    (0.9, 254.0, 220.0, 86.0),
    (1.0, 255.0, 255.0, 240.0),
];

fn heat_rgb(level: f32) -> [u8; 3] {
    let level = level.clamp(0.0, 1.0);
    let upper = HEAT_STOPS.iter().position(|stop| stop.0 >= level).unwrap_or(HEAT_STOPS.len() - 1).max(1);
    let (a, b) = (HEAT_STOPS[upper - 1], HEAT_STOPS[upper]);
    let t = (level - a.0) / (b.0 - a.0);
    let channel = |from: f32, to: f32| (from + (to - from) * t) as u8;
    [channel(a.1, b.1), channel(a.2, b.2), channel(a.3, b.3)]
}

pub(crate) struct SpectrumView {
    /// Smoothed level of each band, 0 to 1.
    levels: Vec<f32>,
    heat: Option<ImageSurface>,
    palette: Vec<[u8; 4]>,
}

impl SpectrumView {
    pub fn new() -> Self {
        let palette = (0..256)
            .map(|i| {
                let [r, g, b] = heat_rgb(i as f32 / 255.0);
                u32::from_be_bytes([0, r, g, b]).to_ne_bytes()
            })
            .collect();
        Self { levels: vec![0.0; BANDS], heat: None, palette }
    }

    pub fn clear(&mut self) {
        self.levels.fill(0.0);
        self.heat = None;
    }

    pub fn is_clear(&self) -> bool {
        self.heat.is_none() && self.levels.iter().all(|level| *level == 0.0)
    }

    /// Takes one analyzer frame in dBFS. Runs between frames, when no cairo
    /// context still holds the heat surface.
    pub fn ingest(&mut self, bands: &[f32], view: EqView) {
        if bands.len() != BANDS {
            return;
        }
        let raw: Vec<f32> = bands
            .iter()
            .enumerate()
            .map(|(band, db)| {
                let tilted = db + TILT_DB_PER_OCTAVE * (band_hz(band) / 1000.0).log2();
                ((tilted - FLOOR_DB) / RANGE_DB).clamp(0.0, 1.0)
            })
            .collect();
        for (level, fresh) in self.levels.iter_mut().zip(&raw) {
            *level = if *fresh > *level { *fresh } else { *level * FALL + *fresh * (1.0 - FALL) };
        }
        if view == EqView::Heat {
            self.push_heat_row(&raw);
        }
    }

    fn push_heat_row(&mut self, row: &[f32]) {
        let background = self.palette[0];
        if self.heat.is_none() {
            let Ok(mut surface) = ImageSurface::create(Format::Rgb24, BANDS as i32, HEAT_ROWS) else { return };
            if let Ok(mut data) = surface.data() {
                data.as_chunks_mut::<4>().0.iter_mut().for_each(|pixel| *pixel = background);
            }
            self.heat = Some(surface);
        }
        let Some(surface) = self.heat.as_mut() else { return };
        let stride = surface.stride() as usize;
        let Ok(mut data) = surface.data() else { return };
        data.copy_within(stride.., 0);
        let start = stride * (HEAT_ROWS as usize - 1);
        for (pixel, level) in data[start..].as_chunks_mut::<4>().0.iter_mut().zip(row) {
            *pixel = self.palette[(*level * 255.0) as usize];
        }
    }

    /// Draws the analyzer in `view` style over `area`, the EQ graph.
    pub fn paint(&self, cr: &Context, view: EqView, area: Rect) {
        let Rect { x, y, w, h } = area;
        let bottom = y + h;
        let point = |band: usize| (x + band as f64 / (BANDS - 1) as f64 * w, bottom - f64::from(self.levels[band]) * h);
        let accent = draw::accent();
        match view {
            EqView::Off => {}
            EqView::Heat => self.paint_heat(cr, area),
            EqView::Line => {
                trace(cr, BANDS, point);
                cr.set_source_rgba(accent.r, accent.g, accent.b, 0.5);
                cr.set_line_width(1.5);
                let _ = cr.stroke();
            }
            EqView::Area => {
                trace(cr, BANDS, point);
                cr.line_to(x + w, bottom);
                cr.line_to(x, bottom);
                cr.close_path();
                let gradient = LinearGradient::new(0.0, y, 0.0, bottom);
                gradient.add_color_stop_rgba(0.0, accent.r, accent.g, accent.b, 0.5);
                gradient.add_color_stop_rgba(1.0, accent.r, accent.g, accent.b, 0.05);
                let _ = cr.set_source(&gradient);
                let _ = cr.fill();
            }
            EqView::Bars => {
                let width = w / BAR_COUNT as f64;
                for bar in 0..BAR_COUNT {
                    let from = bar * BANDS / BAR_COUNT;
                    let to = ((bar + 1) * BANDS / BAR_COUNT).max(from + 1);
                    let level = self.levels[from..to].iter().copied().fold(0.0, f32::max);
                    let bar_h = f64::from(level) * h;
                    cr.rectangle(x + bar as f64 * width + 1.0, bottom - bar_h, (width - 2.0).max(1.0), bar_h);
                }
                cr.set_source_rgba(accent.r, accent.g, accent.b, 0.34);
                let _ = cr.fill();
            }
        }
    }

    /// Stretches the heat map, one pixel per band and row, over `area`.
    fn paint_heat(&self, cr: &Context, area: Rect) {
        let Some(surface) = &self.heat else { return };
        let _ = cr.save();
        cr.translate(area.x, area.y);
        cr.scale(area.w / BANDS as f64, area.h / f64::from(HEAT_ROWS));
        if cr.set_source_surface(surface, 0.0, 0.0).is_ok() {
            let source = cr.source();
            source.set_extend(Extend::Pad);
            source.set_filter(Filter::Bilinear);
            let _ = cr.paint();
        }
        // The context must not keep the surface, or the next row cannot be written.
        cr.set_source_rgb(0.0, 0.0, 0.0);
        let _ = cr.restore();
    }
}

fn trace(cr: &Context, count: usize, point: impl Fn(usize) -> (f64, f64)) {
    for band in 0..count {
        let (px, py) = point(band);
        if band == 0 {
            cr.move_to(px, py);
        } else {
            cr.line_to(px, py);
        }
    }
}

pub(crate) fn self_test() -> Result<(), String> {
    if heat_rgb(0.0) != [14, 16, 21] || heat_rgb(1.0) != [255, 255, 240] {
        return Err("heat palette ends".into());
    }
    let mut view = SpectrumView::new();
    view.ingest(&[-20.0; BANDS], EqView::Heat);
    view.ingest(&[-20.0; BANDS], EqView::Heat);
    if view.heat.is_none() || view.levels.iter().all(|level| *level <= 0.0) {
        return Err("heat row not written".into());
    }
    view.ingest(&[-20.0; 3], EqView::Heat);
    view.clear();
    if !view.is_clear() {
        return Err("spectrum clear".into());
    }
    Ok(())
}
