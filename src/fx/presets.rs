//! Factory presets. A preset writes an effect's values and nothing else, and the
//! one shown as active is found by comparing values, so a moved slider simply
//! reads as Custom. `extra` carries what the sliders do not show:
//! reverb damping, pre-delay and early level; compressor attack and release;
//! delay tone and stereo spread; flanger and phaser feedback (0.5 is none).

use crate::config::{BandType, EffectKind, EffectSlot, EqBand, ParamEq};
use crate::i18n::Lang;

type Band = (f64, f64, f64);

struct EqShape {
    low: BandType,
    high: BandType,
    bands: [Band; 6],
}

pub struct Preset {
    /// In `LANGS` order: Japanese, English, Chinese, German, French, Spanish, Italian.
    names: [&'static str; 7],
    params: [f64; 3],
    extra: [f64; 3],
    eq: Option<EqShape>,
}

impl Preset {
    pub fn name(&self, lang: Lang) -> &'static str {
        let index = crate::i18n::LANGS.iter().position(|item| *item == lang).unwrap_or(1);
        self.names[index]
    }

    fn eq_values(&self) -> Option<ParamEq> {
        let shape = self.eq.as_ref()?;
        let mut eq = ParamEq::new(6);
        eq.low = shape.low;
        eq.high = shape.high;
        eq.bypass = false;
        eq.bands = shape.bands.iter().map(|&(freq, gain_db, q)| EqBand { freq, gain_db, q, on: true }).collect();
        Some(eq)
    }

    pub fn apply(&self, slot: &mut EffectSlot) {
        slot.params = self.params;
        slot.extra = self.extra;
        if slot.kind == EffectKind::Eq {
            slot.eq = self.eq_values();
        }
    }

    fn matches(&self, slot: &EffectSlot) -> bool {
        match slot.kind {
            EffectKind::Eq => slot.eq == self.eq_values(),
            EffectKind::Pitcher => slot.params == self.params,
            _ => slot.params == self.params && slot.extra == self.extra,
        }
    }
}

/// The preset whose values the slot holds, if any.
pub fn active(slot: &EffectSlot) -> Option<&'static Preset> {
    of(slot.kind).iter().find(|preset| preset.matches(slot))
}

const fn fx(names: [&'static str; 7], params: [f64; 3], extra: [f64; 3]) -> Preset {
    Preset { names, params, extra, eq: None }
}

/// A plain shift: the formants move with the pitch, as on a sped-up tape.
const fn semitones(names: [&'static str; 7], value: f64) -> Preset {
    let pitch = value / 24.0 + 0.5;
    fx(names, [pitch, pitch, 0.5], [0.5; 3])
}

/// A natural voice: `pitch` and `formant` in semitones, set apart, and a brightness
/// from 0 (darker) through 0.5 (flat) to 1 (brighter).
///
/// Measured adult voices: women speak about 9 semitones above men, and their vocal
/// tract is about 15 to 20% shorter, so the formants sit about 3 semitones higher.
/// The full pitch gap sounds forced, so the voice presets move about two thirds of
/// it. Children have formants higher still.
const fn voice(names: [&'static str; 7], pitch: f64, formant: f64, brightness: f64) -> Preset {
    fx(names, [pitch / 24.0 + 0.5, formant / 24.0 + 0.5, brightness], [0.5; 3])
}

const fn eq(names: [&'static str; 7], low: BandType, high: BandType, bands: [Band; 6]) -> Preset {
    Preset { names, params: [0.5; 3], extra: [0.5; 3], eq: Some(EqShape { low, high, bands }) }
}

pub fn of(kind: EffectKind) -> &'static [Preset] {
    match kind {
        EffectKind::Reverb => &REVERB,
        EffectKind::Comp => &COMP,
        EffectKind::Eq => &EQ,
        EffectKind::Pitcher => &PITCHER,
        EffectKind::Flanger => &FLANGER,
        EffectKind::Phaser => &PHASER,
        EffectKind::Delay => &DELAY,
        EffectKind::Vst => &[],
    }
}

static REVERB: [Preset; 10] = [
    fx(["部屋", "Room", "房间", "Zimmer", "Pièce", "Habitación", "Stanza"], [0.38, 0.28, 0.22], [0.55, 0.12, 0.6]),
    fx(["バスルーム", "Bathroom", "浴室", "Badezimmer", "Salle de bain", "Baño", "Bagno"], [0.16, 0.38, 0.3], [0.12, 0.05, 0.75]),
    fx(["廊下", "Hallway", "走廊", "Flur", "Couloir", "Pasillo", "Corridoio"], [0.5, 0.42, 0.28], [0.35, 0.2, 0.7]),
    fx(["スタジオ", "Studio", "录音室", "Studio", "Studio", "Estudio", "Studio"], [0.3, 0.18, 0.16], [0.7, 0.15, 0.45]),
    fx(["クラブ", "Club", "俱乐部", "Club", "Club", "Club", "Club"], [0.55, 0.38, 0.22], [0.72, 0.25, 0.55]),
    fx(["コンサートホール", "Concert hall", "音乐厅", "Konzertsaal", "Salle de concert", "Sala de conciertos", "Sala da concerto"], [0.75, 0.62, 0.28], [0.55, 0.45, 0.4]),
    fx(["教会", "Church", "教堂", "Kirche", "Église", "Iglesia", "Chiesa"], [0.85, 0.8, 0.32], [0.42, 0.6, 0.35]),
    fx(["大聖堂", "Cathedral", "大教堂", "Kathedrale", "Cathédrale", "Catedral", "Cattedrale"], [1.0, 0.92, 0.36], [0.5, 0.75, 0.3]),
    fx(["プレート", "Plate", "板式", "Platte", "Plaque", "Placa", "Piastra"], [0.42, 0.52, 0.25], [0.18, 0.1, 0.05]),
    fx(["洞窟", "Cave", "洞穴", "Höhle", "Grotte", "Cueva", "Grotta"], [0.8, 0.72, 0.38], [0.6, 0.4, 0.6]),
];

static COMP: [Preset; 5] = [
    fx(["ボイス", "Voice", "人声", "Stimme", "Voix", "Voz", "Voce"], [0.55, 0.16, 0.2], [0.5, 0.55, 0.5]),
    fx(["ソフト", "Gentle glue", "柔和", "Sanft", "Douce", "Suave", "Morbida"], [0.7, 0.06, 0.08], [0.65, 0.6, 0.5]),
    fx(["放送", "Broadcast", "广播", "Rundfunk", "Radio", "Radio", "Radio"], [0.45, 0.26, 0.3], [0.4, 0.5, 0.5]),
    fx(["リミッター", "Limiter", "限幅器", "Limiter", "Limiteur", "Limitador", "Limiter"], [0.85, 1.0, 0.05], [0.05, 0.35, 0.5]),
    fx(["パンチ", "Punch", "冲击", "Punch", "Punch", "Pegada", "Punch"], [0.5, 0.21, 0.2], [0.85, 0.35, 0.5]),
];

static PITCHER: [Preset; 10] = [
    voice(["男声→女声", "Male to female", "男声变女声", "Mann zu Frau", "Homme vers femme", "Hombre a mujer", "Uomo a donna"], 6.0, 3.0, 0.6),
    voice(["男声→少女", "Male to girl", "男声变少女", "Mann zu Mädchen", "Homme vers fille", "Hombre a chica", "Uomo a ragazza"], 9.0, 4.5, 0.62),
    voice(["女声→男声", "Female to male", "女声变男声", "Frau zu Mann", "Femme vers homme", "Mujer a hombre", "Donna a uomo"], -6.0, -3.0, 0.42),
    voice(["少女→男声", "Girl to male", "少女变男声", "Mädchen zu Mann", "Fille vers homme", "Chica a hombre", "Ragazza a uomo"], -9.0, -4.5, 0.4),
    semitones(["1オクターブ下", "Octave down", "降八度", "Oktave tiefer", "Octave en dessous", "Octava abajo", "Ottava sotto"], -12.0),
    semitones(["低い声", "Deep voice", "低沉嗓音", "Tiefe Stimme", "Voix grave", "Voz grave", "Voce profonda"], -5.0),
    semitones(["明るい声", "Bright voice", "明亮嗓音", "Helle Stimme", "Voix claire", "Voz brillante", "Voce chiara"], 4.0),
    semitones(["5度上", "Fifth up", "升五度", "Quinte höher", "Quinte au-dessus", "Quinta arriba", "Quinta sopra"], 7.0),
    semitones(["チップマンク", "Chipmunk", "花栗鼠", "Streifenhörnchen", "Écureuil", "Ardilla", "Scoiattolo"], 9.0),
    semitones(["1オクターブ上", "Octave up", "升八度", "Oktave höher", "Octave au-dessus", "Octava arriba", "Ottava sopra"], 12.0),
];

static FLANGER: [Preset; 4] = [
    fx(["控えめ", "Subtle", "轻微", "Dezent", "Discret", "Sutil", "Leggero"], [0.03, 0.35, 0.3], [0.55, 0.5, 0.5]),
    fx(["ジェット", "Jet", "喷气", "Jet", "Jet", "Jet", "Jet"], [0.06, 0.95, 0.5], [0.9, 0.5, 0.5]),
    fx(["コーラス風", "Chorus", "合唱", "Chorus", "Chorus", "Coro", "Chorus"], [0.15, 0.25, 0.5], [0.5, 0.5, 0.5]),
    fx(["メタリック", "Metallic", "金属", "Metallisch", "Métallique", "Metálico", "Metallico"], [0.25, 0.6, 0.5], [0.95, 0.5, 0.5]),
];

static PHASER: [Preset; 4] = [
    fx(["スロー", "Slow sweep", "慢扫", "Langsam", "Lent", "Lento", "Lento"], [0.04, 0.7, 0.5], [0.6, 0.5, 0.5]),
    fx(["クラシック", "Classic", "经典", "Klassisch", "Classique", "Clásico", "Classico"], [0.15, 0.55, 0.5], [0.7, 0.5, 0.5]),
    fx(["ファスト", "Fast swirl", "快旋", "Schnell", "Rapide", "Rápido", "Veloce"], [0.7, 0.45, 0.5], [0.6, 0.5, 0.5]),
    fx(["ディープ", "Deep", "深邃", "Tief", "Profond", "Profundo", "Profondo"], [0.08, 1.0, 0.6], [0.85, 0.5, 0.5]),
];

static DELAY: [Preset; 5] = [
    fx(["スラップバック", "Slapback", "拍回", "Slapback", "Slapback", "Slapback", "Slapback"], [0.1, 0.05, 0.3], [0.7, 0.5, 0.5]),
    fx(["リズム", "Rhythmic", "节奏", "Rhythmisch", "Rythmique", "Rítmico", "Ritmico"], [0.42, 0.3, 0.25], [0.7, 0.5, 0.5]),
    fx(["ピンポン", "Ping-pong", "乒乓", "Ping-Pong", "Ping-pong", "Ping-pong", "Ping-pong"], [0.33, 0.45, 0.3], [0.75, 1.0, 0.5]),
    fx(["ダブ", "Dub", "Dub", "Dub", "Dub", "Dub", "Dub"], [0.5, 0.65, 0.35], [0.3, 0.85, 0.5]),
    fx(["ロングエコー", "Long echo", "长回声", "Langes Echo", "Écho long", "Eco largo", "Eco lungo"], [0.85, 0.4, 0.25], [0.55, 0.5, 0.5]),
];

static EQ: [Preset; 6] = [
    eq(
        ["声の明瞭さ", "Voice clarity", "人声清晰", "Klare Stimme", "Voix claire", "Voz clara", "Voce chiara"],
        BandType::HighPass,
        BandType::Shelf,
        [(90.0, 0.0, 0.71), (250.0, -2.5, 1.0), (1000.0, 0.0, 1.0), (3000.0, 3.0, 1.0), (6000.0, 1.0, 1.0), (11000.0, 2.5, 0.71)],
    ),
    eq(
        ["暖かい", "Warm", "温暖", "Warm", "Chaud", "Cálido", "Caldo"],
        BandType::Shelf,
        BandType::Shelf,
        [(150.0, 3.0, 0.71), (400.0, 1.0, 1.0), (1000.0, 0.0, 1.0), (3000.0, -1.0, 1.0), (6000.0, -1.5, 1.0), (10000.0, -3.0, 0.71)],
    ),
    eq(
        ["明るい", "Bright", "明亮", "Hell", "Brillant", "Brillante", "Brillante"],
        BandType::Shelf,
        BandType::Shelf,
        [(100.0, -1.0, 0.71), (400.0, -1.0, 1.0), (1000.0, 0.0, 1.0), (3500.0, 2.0, 1.0), (7000.0, 2.5, 1.0), (12000.0, 4.0, 0.71)],
    ),
    eq(
        ["電話", "Telephone", "电话", "Telefon", "Téléphone", "Teléfono", "Telefono"],
        BandType::HighPass,
        BandType::LowPass,
        [(400.0, 0.0, 0.9), (700.0, 0.0, 1.0), (1500.0, 5.0, 0.8), (2200.0, 2.0, 1.0), (2800.0, 0.0, 1.0), (3200.0, 0.0, 0.9)],
    ),
    eq(
        ["低音強調", "Bass boost", "低音增强", "Bass-Boost", "Basses renforcées", "Más graves", "Bassi potenziati"],
        BandType::Shelf,
        BandType::Shelf,
        [(110.0, 6.0, 0.71), (250.0, 1.0, 1.0), (500.0, -1.5, 1.0), (2000.0, 0.0, 1.0), (5000.0, 0.0, 1.0), (10000.0, 0.0, 0.71)],
    ),
    eq(
        ["こもり除去", "De-mud", "去浑浊", "Entmulmen", "Clarifier", "Limpiar medios", "Pulisci medi"],
        BandType::HighPass,
        BandType::Shelf,
        [(60.0, 0.0, 0.71), (300.0, -4.0, 1.2), (600.0, -1.5, 1.2), (2000.0, 0.0, 1.0), (5000.0, 1.0, 1.0), (10000.0, 1.0, 0.71)],
    ),
];

pub fn self_test() -> Result<(), String> {
    for kind in EffectKind::ALL {
        if of(kind).is_empty() {
            return Err(format!("no presets for {}", kind.key()));
        }
        for preset in of(kind) {
            let mut slot = EffectSlot::new(kind);
            preset.apply(&mut slot);
            if !active(&slot).is_some_and(|found| std::ptr::eq(found, preset)) {
                return Err(format!("preset {} not recognised", preset.name(Lang::En)));
            }
            if slot.params.iter().chain(&slot.extra).any(|value| !(0.0..=1.0).contains(value)) {
                return Err(format!("preset {} out of range", preset.name(Lang::En)));
            }
        }
    }
    let mut moved = EffectSlot::new(EffectKind::Reverb);
    of(EffectKind::Reverb)[0].apply(&mut moved);
    moved.params[1] += 0.01;
    if active(&moved).is_some() {
        return Err("moved slider still matches a preset".into());
    }
    Ok(())
}
