//! Rasterized logo and flag icons. The SVGs are too complex for the panel's
//! path parser, so `rsvg-convert` produced these PNGs.

use crate::i18n::Lang;

pub fn icon_png(pink: bool, size: u32) -> &'static [u8] {
    match (pink, size) {
        (true, 48) => include_bytes!("../assets/icons/pink-48.png"),
        (true, 128) => include_bytes!("../assets/icons/pink-128.png"),
        (true, 256) => include_bytes!("../assets/icons/pink-256.png"),
        (_, 48) => include_bytes!("../assets/icons/blue-48.png"),
        (_, 128) => include_bytes!("../assets/icons/blue-128.png"),
        _ => include_bytes!("../assets/icons/blue-256.png"),
    }
}

pub fn flag_png(lang: Lang) -> &'static [u8] {
    match lang {
        Lang::De => include_bytes!("../assets/flags/de.png"),
        Lang::Fr => include_bytes!("../assets/flags/fr.png"),
        Lang::Sc => include_bytes!("../assets/flags/cn.png"),
        Lang::Ja => include_bytes!("../assets/flags/jp.png"),
        Lang::Es => include_bytes!("../assets/flags/es.png"),
        Lang::It => include_bytes!("../assets/flags/it.png"),
        Lang::En => include_bytes!("../assets/flags/en.png"),
    }
}
