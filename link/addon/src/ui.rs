//! The overlay panel: what it shows, how it is drawn, and what a press means.
//!
//! The panel is drawn on the CPU into an RGBA buffer that SteamVR takes as it
//! is. Drawing happens only when something visible changed: every frame starts
//! as a [`View`], a plain description of the picture, and a view equal to the
//! one already on screen is not drawn again.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Context;
use frame_link_core::{mask_has, LinkState};
use tiny_skia::{FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform};

use crate::config::device_name;
use crate::model::{Action, DeviceState, Kind, Snapshot, STREAMS};

/// Panel size in pixels. Pointer positions use the same scale.
pub const WIDTH: u32 = 1200;
pub const HEIGHT: u32 = 760;
/// Edge of the square dashboard icon.
pub const THUMBNAIL: u32 = 256;

const MARGIN: f32 = 28.0;
const TILE_GAP: f32 = 16.0;
const TILE_H: f32 = 244.0;
const TILE_PAD: f32 = 20.0;
const TILE_RADIUS: f32 = 18.0;
/// Top of the two groups: their title line, then their tiles 40 px below.
const GROUP_TOP: [f32; 2] = [132.0, 440.0];
const GROUP_TILES: f32 = 40.0;
/// The connection box in the header.
const STATUS_W: f32 = 500.0;
const STATUS_H: f32 = 96.0;
/// The language button, left of the connection box, and the menu it opens.
const LANG_BUTTON: Rect = Rect { x: WIDTH as f32 - MARGIN - STATUS_W - 20.0 - 152.0, y: 22.0, w: 152.0, h: 42.0 };
const MENU_W: f32 = 236.0;
const MENU_PAD: f32 = 8.0;
const MENU_ROW_H: f32 = 48.0;

/// An opaque color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rgb(u8, u8, u8);

// The headset mixer's surfaces and text colors, so both panels read as one family.
const BG: Rgb = Rgb(0x0e, 0x10, 0x15);
const CARD: Rgb = Rgb(0x17, 0x1a, 0x21);
const INSET: Rgb = Rgb(0x1f, 0x23, 0x2b);
const CONTROL: Rgb = Rgb(0x3b, 0x41, 0x4d);
const TEXT: Rgb = Rgb(0xf2, 0xf3, 0xf5);
const MUTED: Rgb = Rgb(0xa3, 0xa9, 0xb4);
const ACCENT: Rgb = Rgb(0x33, 0xc4, 0xff);
const ACCENT_INK: Rgb = Rgb(0x0e, 0x10, 0x15);
const GOOD: Rgb = Rgb(0x4a, 0xde, 0x80);
const WAITING: Rgb = Rgb(0xf0, 0xc1, 0x4a);
const WARN: Rgb = Rgb(0xff, 0x8a, 0x80);
const WHITE: Rgb = Rgb(0xff, 0xff, 0xff);

/// `top` laid over `base` at `amount`.
fn mix(top: Rgb, base: Rgb, amount: f32) -> Rgb {
    let channel = |top: u8, base: u8| (f32::from(base) + (f32::from(top) - f32::from(base)) * amount).round() as u8;
    Rgb(channel(top.0, base.0), channel(top.1, base.1), channel(top.2, base.2))
}

/// The languages the panel speaks: the same seven as the headset mixer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Ja,
    Sc,
    De,
    Fr,
    Es,
    It,
}

impl Lang {
    /// Every language, in the order of the language menu.
    pub const ALL: [Lang; 7] = [Lang::En, Lang::Ja, Lang::Sc, Lang::De, Lang::Fr, Lang::Es, Lang::It];

    /// The language chosen in the overlay (its code, as saved in the
    /// settings), or the system's while none was chosen.
    #[must_use]
    pub fn from_setting(code: &str) -> Self {
        Self::ALL.into_iter().find(|lang| lang.code() == code).unwrap_or_else(Self::detect)
    }

    /// The system's language where the panel speaks it, English otherwise.
    #[must_use]
    pub fn detect() -> Self {
        sys_locale::get_locale().map_or(Lang::En, |locale| Self::from_locale(&locale))
    }

    /// The language of a locale name such as `de-AT`, `ja_JP.UTF-8` or `zh-CN`.
    fn from_locale(locale: &str) -> Self {
        let lower = locale.to_ascii_lowercase();
        match lower.split(['-', '_', '.', '@']).next().unwrap_or("") {
            "ja" => Lang::Ja,
            "zh" => Lang::Sc,
            "de" => Lang::De,
            "fr" => Lang::Fr,
            "es" => Lang::Es,
            "it" => Lang::It,
            _ => Lang::En,
        }
    }

    fn text(self) -> &'static Text {
        match self {
            Lang::En => &EN,
            Lang::Ja => &JA,
            Lang::Sc => &SC,
            Lang::De => &DE,
            Lang::Fr => &FR,
            Lang::Es => &ES,
            Lang::It => &IT,
        }
    }

    /// The code the choice is saved under: the headset mixer's codes.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ja => "ja",
            Lang::Sc => "sc",
            Lang::De => "de",
            Lang::Fr => "fr",
            Lang::Es => "es",
            Lang::It => "it",
        }
    }

    /// The language's name in that language, for the language menu.
    fn name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Ja => "日本語",
            Lang::Sc => "简体中文",
            Lang::De => "Deutsch",
            Lang::Fr => "Français",
            Lang::Es => "Español",
            Lang::It => "Italiano",
        }
    }

    /// Font files with Chinese, Japanese and Korean, the one for this
    /// language's script first. The same character is drawn differently in
    /// Japanese and in Chinese, so the first font that has it should be the
    /// reader's own. Everyone else gets the Chinese font first, because it
    /// alone can write both names in the language menu.
    fn fallback_files(self) -> &'static [&'static str] {
        match self {
            Lang::Ja => &[
                "YuGothM.ttc",
                "meiryo.ttc",
                "NotoSansCJK-Regular.ttc",
                "NotoSansCJKjp-Regular.otf",
                "msyh.ttc",
                "NotoSansCJKsc-Regular.otf",
                "msgothic.ttc",
                "malgun.ttf",
                "wqy-microhei.ttc",
                "DroidSansFallbackFull.ttf",
            ],
            _ => &[
                "msyh.ttc",
                "NotoSansCJK-Regular.ttc",
                "NotoSansCJKsc-Regular.otf",
                "wqy-microhei.ttc",
                "YuGothM.ttc",
                "meiryo.ttc",
                "NotoSansCJKjp-Regular.otf",
                "msgothic.ttc",
                "malgun.ttf",
                "DroidSansFallbackFull.ttf",
            ],
        }
    }
}

/// Every string on the panel. `{peer}`, `{addr}` and `{rtt}` are filled in.
struct Text {
    title: &'static str,
    hint: &'static str,
    connected: &'static str,
    connected_detail: &'static str,
    searching: &'static str,
    searching_detail: &'static str,
    busy: &'static str,
    busy_detail: &'static str,
    version: &'static str,
    version_detail: &'static str,
    not_allowed: &'static str,
    not_allowed_detail: &'static str,
    refused: &'static str,
    mics: &'static str,
    mics_note: &'static str,
    outputs: &'static str,
    outputs_note: &'static str,
    on: &'static str,
    off: &'static str,
    device_missing: &'static str,
    device_failed: &'static str,
    switched_off: &'static str,
    waiting: &'static str,
    not_offered: &'static str,
    not_in_mixer: &'static str,
    receiving: &'static str,
    sending: &'static str,
    ready: &'static str,
}

const EN: Text = Text {
    title: "Frame Audio Link",
    hint: "A stream that is switched off uses no network bandwidth.",
    connected: "Connected to {peer}",
    connected_detail: "{addr} · round trip {rtt} ms",
    searching: "Looking for the headset…",
    searching_detail: "Start wireless streaming on the Steam Frame and open Frame Audio Mixer there.",
    busy: "The headset is taken",
    busy_detail: "Another PC is linked to the headset. Close Frame Audio Link on that PC.",
    version: "The versions do not match",
    version_detail: "Update Frame Audio Mixer on the headset or this add-on, so both are current.",
    not_allowed: "The headset turned this PC down",
    not_allowed_detail: "It only links to the PC it streams from. Start wireless streaming from this PC.",
    refused: "The headset refused the link",
    mics: "Microphones from the headset",
    mics_note: "B1–B4 of the mixer, as microphones on this PC",
    outputs: "PC outputs to the headset",
    outputs_note: "PC1–PC4, as channels in the mixer",
    on: "On",
    off: "Off",
    device_missing: "Sound device is not installed",
    device_failed: "Sound device could not be opened",
    switched_off: "Off · uses no bandwidth",
    waiting: "Waiting for the headset",
    not_offered: "Not offered by the headset",
    not_in_mixer: "Not added in the mixer yet · nothing is sent",
    receiving: "Receiving sound",
    sending: "Sending sound",
    ready: "Ready · silent",
};

const DE: Text = Text {
    title: "Frame Audio Link",
    hint: "Ein ausgeschalteter Stream belegt keine Bandbreite.",
    connected: "Verbunden mit {peer}",
    connected_detail: "{addr} · Antwortzeit {rtt} ms",
    searching: "Headset wird gesucht…",
    searching_detail: "Starte das kabellose Streaming am Steam Frame und öffne dort Frame Audio Mixer.",
    busy: "Das Headset ist belegt",
    busy_detail: "Ein anderer PC ist mit dem Headset verbunden. Beende Frame Audio Link auf jenem PC.",
    version: "Die Versionen passen nicht zusammen",
    version_detail: "Aktualisiere Frame Audio Mixer auf dem Headset oder dieses Add-on, bis beide aktuell sind.",
    not_allowed: "Das Headset hat diesen PC abgelehnt",
    not_allowed_detail: "Es verbindet sich nur mit dem PC, von dem es streamt. Starte das Streaming von diesem PC.",
    refused: "Das Headset lehnt die Verbindung ab",
    mics: "Mikrofone vom Headset",
    mics_note: "B1–B4 des Mixers, als Mikrofone an diesem PC",
    outputs: "PC-Ausgänge zum Headset",
    outputs_note: "PC1–PC4, als Kanäle im Mixer",
    on: "An",
    off: "Aus",
    device_missing: "Audiogerät ist nicht installiert",
    device_failed: "Audiogerät ließ sich nicht öffnen",
    switched_off: "Aus · belegt keine Bandbreite",
    waiting: "Wartet auf das Headset",
    not_offered: "Vom Headset nicht angeboten",
    not_in_mixer: "Noch nicht im Mixer · nichts wird gesendet",
    receiving: "Empfängt Ton",
    sending: "Sendet Ton",
    ready: "Bereit · still",
};

const JA: Text = Text {
    title: "Frame Audio Link",
    hint: "オフにしたストリームは通信帯域を使いません。",
    connected: "{peer} に接続中",
    connected_detail: "{addr} · 往復 {rtt} ms",
    searching: "ヘッドセットを探しています…",
    searching_detail: "Steam Frame でワイヤレスストリーミングを始め、Frame Audio Mixer を開いてください。",
    busy: "ヘッドセットは使用中です",
    busy_detail: "別の PC がヘッドセットに接続しています。その PC の Frame Audio Link を終了してください。",
    version: "バージョンが合いません",
    version_detail: "ヘッドセットの Frame Audio Mixer かこのアドオンを更新して、両方を最新にしてください。",
    not_allowed: "ヘッドセットがこの PC を拒否しました",
    not_allowed_detail: "ヘッドセットはストリーミング元の PC とだけ接続します。この PC からストリーミングを始めてください。",
    refused: "ヘッドセットが接続を拒否しました",
    mics: "ヘッドセットからのマイク",
    mics_note: "ミキサーの B1–B4 が、この PC のマイクになります",
    outputs: "ヘッドセットへの PC 出力",
    outputs_note: "PC1–PC4 が、ミキサーのチャンネルになります",
    on: "オン",
    off: "オフ",
    device_missing: "サウンドデバイスが未インストールです",
    device_failed: "サウンドデバイスを開けませんでした",
    switched_off: "オフ · 帯域を使いません",
    waiting: "ヘッドセットを待っています",
    not_offered: "ヘッドセットから提供されていません",
    not_in_mixer: "ミキサーに未追加 · 何も送信しません",
    receiving: "音声を受信中",
    sending: "音声を送信中",
    ready: "待機中 · 無音",
};

const SC: Text = Text {
    title: "Frame Audio Link",
    hint: "关闭的音频流不占用网络带宽。",
    connected: "已连接到 {peer}",
    connected_detail: "{addr} · 往返 {rtt} ms",
    searching: "正在查找头显…",
    searching_detail: "请在 Steam Frame 上开始无线串流，并打开 Frame Audio Mixer。",
    busy: "头显已被占用",
    busy_detail: "另一台 PC 已连接到头显。请关闭那台 PC 上的 Frame Audio Link。",
    version: "版本不一致",
    version_detail: "请更新头显上的 Frame Audio Mixer 或此插件，使两者均为最新版本。",
    not_allowed: "头显拒绝了这台 PC",
    not_allowed_detail: "头显只连接正在向它串流的 PC。请从这台 PC 开始串流。",
    refused: "头显拒绝了连接",
    mics: "来自头显的麦克风",
    mics_note: "混音器的 B1–B4，作为这台 PC 的麦克风",
    outputs: "发送到头显的 PC 输出",
    outputs_note: "PC1–PC4，作为混音器中的通道",
    on: "开",
    off: "关",
    device_missing: "未安装声音设备",
    device_failed: "无法打开声音设备",
    switched_off: "已关闭 · 不占用带宽",
    waiting: "正在等待头显",
    not_offered: "头显未提供",
    not_in_mixer: "尚未添加到混音器 · 不发送任何内容",
    receiving: "正在接收声音",
    sending: "正在发送声音",
    ready: "就绪 · 静音",
};

const FR: Text = Text {
    title: "Frame Audio Link",
    hint: "Un flux désactivé n'utilise aucune bande passante.",
    connected: "Connecté à {peer}",
    connected_detail: "{addr} · aller-retour {rtt} ms",
    searching: "Recherche du casque…",
    searching_detail: "Lancez le streaming sans fil sur le Steam Frame et ouvrez-y Frame Audio Mixer.",
    busy: "Le casque est occupé",
    busy_detail: "Un autre PC est relié au casque. Fermez Frame Audio Link sur ce PC.",
    version: "Les versions ne correspondent pas",
    version_detail: "Mettez à jour Frame Audio Mixer sur le casque ou ce module, pour que les deux soient à jour.",
    not_allowed: "Le casque a refusé ce PC",
    not_allowed_detail: "Il ne se relie qu'au PC depuis lequel il streame. Lancez le streaming depuis ce PC.",
    refused: "Le casque a refusé la liaison",
    mics: "Micros du casque",
    mics_note: "B1–B4 du mixeur, comme micros sur ce PC",
    outputs: "Sorties du PC vers le casque",
    outputs_note: "PC1–PC4, comme canaux dans le mixeur",
    on: "Actif",
    off: "Coupé",
    device_missing: "Périphérique audio non installé",
    device_failed: "Impossible d'ouvrir le périphérique audio",
    switched_off: "Coupé · aucune bande passante utilisée",
    waiting: "En attente du casque",
    not_offered: "Non proposé par le casque",
    not_in_mixer: "Pas encore dans le mixeur · rien n'est envoyé",
    receiving: "Réception du son",
    sending: "Envoi du son",
    ready: "Prêt · silencieux",
};

const ES: Text = Text {
    title: "Frame Audio Link",
    hint: "Un flujo apagado no usa ancho de banda.",
    connected: "Conectado a {peer}",
    connected_detail: "{addr} · ida y vuelta {rtt} ms",
    searching: "Buscando el visor…",
    searching_detail: "Inicia el streaming inalámbrico en el Steam Frame y abre allí Frame Audio Mixer.",
    busy: "El visor está ocupado",
    busy_detail: "Otro PC está enlazado con el visor. Cierra Frame Audio Link en ese PC.",
    version: "Las versiones no coinciden",
    version_detail: "Actualiza Frame Audio Mixer en el visor o este complemento, hasta que ambos estén al día.",
    not_allowed: "El visor ha rechazado este PC",
    not_allowed_detail: "Solo se enlaza con el PC desde el que hace streaming. Inicia el streaming desde este PC.",
    refused: "El visor ha rechazado el enlace",
    mics: "Micrófonos del visor",
    mics_note: "B1–B4 del mezclador, como micrófonos en este PC",
    outputs: "Salidas del PC hacia el visor",
    outputs_note: "PC1–PC4, como canales en el mezclador",
    on: "Activo",
    off: "Apagado",
    device_missing: "El dispositivo de sonido no está instalado",
    device_failed: "No se pudo abrir el dispositivo de sonido",
    switched_off: "Apagado · no usa ancho de banda",
    waiting: "Esperando al visor",
    not_offered: "El visor no lo ofrece",
    not_in_mixer: "Aún no está en el mezclador · no se envía nada",
    receiving: "Recibiendo sonido",
    sending: "Enviando sonido",
    ready: "Listo · en silencio",
};

const IT: Text = Text {
    title: "Frame Audio Link",
    hint: "Uno stream spento non usa banda di rete.",
    connected: "Connesso a {peer}",
    connected_detail: "{addr} · andata e ritorno {rtt} ms",
    searching: "Ricerca del visore…",
    searching_detail: "Avvia lo streaming wireless sullo Steam Frame e apri lì Frame Audio Mixer.",
    busy: "Il visore è occupato",
    busy_detail: "Un altro PC è collegato al visore. Chiudi Frame Audio Link su quel PC.",
    version: "Le versioni non corrispondono",
    version_detail: "Aggiorna Frame Audio Mixer sul visore o questo componente, finché entrambi sono aggiornati.",
    not_allowed: "Il visore ha rifiutato questo PC",
    not_allowed_detail: "Si collega solo al PC da cui fa streaming. Avvia lo streaming da questo PC.",
    refused: "Il visore ha rifiutato il collegamento",
    mics: "Microfoni dal visore",
    mics_note: "B1–B4 del mixer, come microfoni su questo PC",
    outputs: "Uscite del PC verso il visore",
    outputs_note: "PC1–PC4, come canali nel mixer",
    on: "Attivo",
    off: "Spento",
    device_missing: "Il dispositivo audio non è installato",
    device_failed: "Impossibile aprire il dispositivo audio",
    switched_off: "Spento · non usa banda",
    waiting: "In attesa del visore",
    not_offered: "Non offerto dal visore",
    not_in_mixer: "Non ancora nel mixer · non viene inviato nulla",
    receiving: "Ricezione audio",
    sending: "Invio audio",
    ready: "Pronto · in silenzio",
};

/// How a line of status text is colored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tone {
    Muted,
    Good,
    Waiting,
    Warn,
}

impl Tone {
    fn color(self) -> Rgb {
        match self {
            Tone::Muted => MUTED,
            Tone::Good => GOOD,
            Tone::Waiting => WAITING,
            Tone::Warn => WARN,
        }
    }
}

/// What the pointer is doing to a tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Touch {
    None,
    Hover,
    Pressed,
}

/// One stream's tile, as drawn.
#[derive(Debug, Clone, PartialEq)]
struct TileView {
    /// "B1" or "PC3".
    label: String,
    /// The headset's name for a microphone, or the PC sound device's name.
    name: String,
    note: &'static str,
    tone: Tone,
    on: bool,
    /// Sound is moving on this stream right now.
    live: bool,
    touch: Touch,
}

/// The connection box, as drawn.
#[derive(Debug, Clone, PartialEq)]
struct StatusView {
    tone: Tone,
    title: String,
    detail: String,
}

/// The open language menu, as drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
struct MenuView {
    /// The row under the pointer, and whether it is held down.
    touched: Option<(usize, Touch)>,
}

/// Everything on the panel. Two equal views are the same picture.
#[derive(Debug, Clone, PartialEq)]
struct View {
    /// The language of every fixed string on the panel.
    lang: Lang,
    status: StatusView,
    /// Microphones, then outputs.
    tiles: [Vec<TileView>; 2],
    language_button: Touch,
    /// `Some` while the language menu is open.
    menu: Option<MenuView>,
}

const KINDS: [Kind; 2] = [Kind::Mic, Kind::Output];

/// What a pointer position means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    /// The tile of one stream.
    Tile(Kind, usize),
    /// The button that opens the language menu.
    LanguageButton,
    /// A row of the open language menu, by its place in [`Lang::ALL`].
    Language(usize),
}

fn status_view(snap: &Snapshot, text: &Text) -> StatusView {
    let link = &snap.link;
    match link.state {
        LinkState::Connected => {
            let rtt = if link.rtt_ms < 1.0 { "<1".to_string() } else { format!("{:.0}", link.rtt_ms) };
            let peer = if link.peer.is_empty() { "Steam Frame" } else { &link.peer };
            StatusView {
                tone: Tone::Good,
                title: text.connected.replace("{peer}", peer),
                detail: text.connected_detail.replace("{addr}", &link.peer_addr).replace("{rtt}", &rtt),
            }
        }
        LinkState::Searching => StatusView { tone: Tone::Waiting, title: text.searching.into(), detail: text.searching_detail.into() },
        LinkState::Refused => {
            let (title, detail) = match link.refused.as_str() {
                "busy" => (text.busy, text.busy_detail.to_string()),
                "version" => (text.version, text.version_detail.to_string()),
                "not-allowed" => (text.not_allowed, text.not_allowed_detail.to_string()),
                // A reason from a newer headset: shown as it came.
                other => (text.refused, other.to_string()),
            };
            StatusView { tone: Tone::Warn, title: title.into(), detail }
        }
    }
}

fn tile_view(snap: &Snapshot, text: &Text, kind: Kind, index: usize, touch: Touch) -> TileView {
    let on = snap.switched_on(kind)[index];
    let connected = snap.link.connected();
    let device = snap.audio.of(kind)[index];
    let live = on && connected && device == DeviceState::Ready && snap.carrying(kind, index);
    // A microphone needs the headset to offer it; an output needs a channel in the mixer.
    let (wanted, unwanted, flowing) = match kind {
        Kind::Mic => (mask_has(snap.link.remote_send, index), text.not_offered, text.receiving),
        Kind::Output => (mask_has(snap.link.remote_recv, index), text.not_in_mixer, text.sending),
    };
    let (note, tone) = match device {
        DeviceState::Missing => (text.device_missing, Tone::Warn),
        DeviceState::Failed => (text.device_failed, Tone::Warn),
        DeviceState::Ready if !on => (text.switched_off, Tone::Muted),
        DeviceState::Ready if !connected => (text.waiting, Tone::Muted),
        DeviceState::Ready if !wanted => (unwanted, Tone::Waiting),
        DeviceState::Ready if live => (flowing, Tone::Good),
        DeviceState::Ready => (text.ready, Tone::Muted),
    };
    let headset_name = snap.link.names.get(index).map(|name| name.trim()).filter(|name| !name.is_empty());
    let name = match (kind, headset_name) {
        (Kind::Mic, Some(name)) => name.to_string(),
        _ => device_name(kind, index),
    };
    TileView { label: format!("{}{}", kind.prefix(), index + 1), name, note, tone, on, live, touch }
}

/// Where the pointer is and what it holds down.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Pointer {
    hover: Option<Target>,
    pressed: Option<Target>,
}

impl Pointer {
    fn touch(self, target: Target) -> Touch {
        if self.pressed == Some(target) {
            Touch::Pressed
        } else if self.hover == Some(target) {
            Touch::Hover
        } else {
            Touch::None
        }
    }
}

fn view(snap: &Snapshot, lang: Lang, pointer: Pointer, menu_open: bool) -> View {
    let text = lang.text();
    let tiles = KINDS.map(|kind| (0..STREAMS).map(|index| tile_view(snap, text, kind, index, pointer.touch(Target::Tile(kind, index)))).collect());
    let menu = menu_open.then(|| MenuView {
        touched: (0..Lang::ALL.len()).map(|row| (row, pointer.touch(Target::Language(row)))).find(|(_, touch)| *touch != Touch::None),
    });
    View { lang, status: status_view(snap, text), tiles, language_button: pointer.touch(Target::LanguageButton), menu }
}

/// A rectangle in panel pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Rect {
    fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

fn group_of(kind: Kind) -> usize {
    match kind {
        Kind::Mic => 0,
        Kind::Output => 1,
    }
}

/// Where the tile of one stream sits. Drawing and hit testing share it.
fn tile_rect(kind: Kind, index: usize) -> Rect {
    let width = (WIDTH as f32 - 2.0 * MARGIN - (STREAMS - 1) as f32 * TILE_GAP) / STREAMS as f32;
    Rect { x: MARGIN + index as f32 * (width + TILE_GAP), y: GROUP_TOP[group_of(kind)] + GROUP_TILES, w: width, h: TILE_H }
}

/// The stream whose tile is under the pointer.
fn tile_at(x: f32, y: f32) -> Option<(Kind, usize)> {
    KINDS.into_iter().flat_map(|kind| (0..STREAMS).map(move |index| (kind, index))).find(|(kind, index)| tile_rect(*kind, *index).contains(x, y))
}

/// The language menu: below its button, with their right edges in line.
fn menu_rect() -> Rect {
    let height = 2.0 * MENU_PAD + Lang::ALL.len() as f32 * MENU_ROW_H;
    Rect { x: LANG_BUTTON.x + LANG_BUTTON.w - MENU_W, y: LANG_BUTTON.y + LANG_BUTTON.h + 8.0, w: MENU_W, h: height }
}

fn menu_row_rect(row: usize) -> Rect {
    let menu = menu_rect();
    Rect { x: menu.x + MENU_PAD, y: menu.y + MENU_PAD + row as f32 * MENU_ROW_H, w: menu.w - 2.0 * MENU_PAD, h: MENU_ROW_H }
}

/// What is under the pointer. The open menu covers the tiles below it, and
/// nothing beside it can be pressed until it is closed.
fn target_at(x: f32, y: f32, menu_open: bool) -> Option<Target> {
    if LANG_BUTTON.contains(x, y) {
        return Some(Target::LanguageButton);
    }
    if menu_open {
        return (0..Lang::ALL.len()).find(|row| menu_row_rect(*row).contains(x, y)).map(Target::Language);
    }
    tile_at(x, y).map(|(kind, index)| Target::Tile(kind, index))
}

/// How a piece of text is set: its weight, its size in pixels and its color.
#[derive(Debug, Clone, Copy)]
struct Style {
    face: Face,
    px: f32,
    color: Rgb,
}

const fn regular(px: f32, color: Rgb) -> Style {
    Style { face: Face::Regular, px, color }
}

const fn bold(px: f32, color: Rgb) -> Style {
    Style { face: Face::Bold, px, color }
}

/// The two weights the panel is set in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Face {
    Regular,
    Bold,
    /// A font with the scripts the main one lacks, by its place in
    /// `Fonts::fallbacks`. Loaded when first needed.
    Fallback(usize),
}

/// A font for Chinese, Japanese and Korean, and whether it was looked for yet.
struct Fallback {
    file: &'static str,
    font: Option<Box<fontdue::Font>>,
    tried: bool,
}

/// System fonts and the glyphs already rasterized from them.
struct Fonts {
    regular: Option<fontdue::Font>,
    bold: Option<fontdue::Font>,
    /// In the order they are asked for a character the main font lacks.
    fallbacks: Vec<Fallback>,
    /// The installed font files, listed when the first fallback is needed.
    files: Option<FontFiles>,
    glyphs: HashMap<(Face, char, u32), (fontdue::Metrics, Vec<u8>)>,
}

impl Fonts {
    /// Loads the system's interface font. Without one the panel still works:
    /// the tiles and switches are drawn and the text is left out.
    fn load(lang: Lang) -> Self {
        let found = FontFiles::scan();
        let regular = found.first(REGULAR_FILES).or_else(|| fc_match("sans")).and_then(|path| load_font(&path));
        let bold = found.first(BOLD_FILES).or_else(|| fc_match("sans:bold")).and_then(|path| load_font(&path));
        if regular.is_none() {
            crate::log::line("no interface font was found; the overlay is drawn without text");
        }
        let mut fonts = Self { regular, bold, fallbacks: Vec::new(), files: Some(found), glyphs: HashMap::new() };
        fonts.set_language(lang);
        fonts
    }

    /// Puts the fallback fonts in the order `lang` reads best. Fonts that are
    /// loaded already stay loaded.
    fn set_language(&mut self, lang: Lang) {
        let mut loaded = std::mem::take(&mut self.fallbacks);
        self.fallbacks = lang
            .fallback_files()
            .iter()
            .map(|file| match loaded.iter().position(|fallback| fallback.file == *file) {
                Some(at) => loaded.swap_remove(at),
                None => Fallback { file, font: None, tried: false },
            })
            .collect();
        // Their places changed, and with them the keys of their glyphs.
        self.glyphs.retain(|(face, _, _), _| !matches!(face, Face::Fallback(_)));
    }

    fn font(&self, face: Face) -> Option<&fontdue::Font> {
        match face {
            Face::Regular => self.regular.as_ref(),
            Face::Bold => self.bold.as_ref().or(self.regular.as_ref()),
            Face::Fallback(index) => self.fallbacks.get(index).and_then(|fallback| fallback.font.as_deref()),
        }
    }

    /// The face that can draw `ch`: the wanted one, or the first fallback
    /// that has the character (Japanese or Chinese text, or a microphone
    /// named in another script on the headset).
    fn face_for(&mut self, face: Face, ch: char) -> Face {
        if self.font(face).is_none_or(|font| font.lookup_glyph_index(ch) != 0) {
            return face;
        }
        for index in 0..self.fallbacks.len() {
            if !self.fallbacks[index].tried {
                let files = self.files.get_or_insert_with(FontFiles::scan);
                let font = files.first(&[self.fallbacks[index].file]).and_then(|path| load_font(&path));
                self.fallbacks[index].font = font.map(Box::new);
                self.fallbacks[index].tried = true;
            }
            if self.fallbacks[index].font.as_ref().is_some_and(|font| font.lookup_glyph_index(ch) != 0) {
                return Face::Fallback(index);
            }
        }
        face
    }

    fn advance(&mut self, face: Face, ch: char, px: f32) -> f32 {
        let face = self.face_for(face, ch);
        self.font(face).map_or(0.0, |font| font.metrics(ch, px).advance_width)
    }

    fn width(&mut self, style: Style, text: &str) -> f32 {
        text.chars().map(|ch| self.advance(style.face, ch, style.px)).sum()
    }

    /// `text`, cut short with an ellipsis when it is wider than `room`.
    fn fit(&mut self, style: Style, text: &str, room: f32) -> String {
        if self.width(style, text) <= room {
            return text.to_string();
        }
        let mut kept: String = text.to_string();
        while kept.pop().is_some() {
            let candidate = format!("{}…", kept.trim_end());
            if self.width(style, &candidate) <= room {
                return candidate;
            }
        }
        String::new()
    }

    /// `text` broken into at most `lines` lines of `room` pixels: at spaces,
    /// and between the characters of Japanese and Chinese, which have none.
    /// What does not fit the last line is cut with an ellipsis.
    fn wrap(&mut self, style: Style, text: &str, room: f32, lines: usize) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut pieces = break_pieces(text).into_iter();
        while let Some(piece) = pieces.next() {
            let joined = match out.last() {
                Some(line) => format!("{line}{}{}", piece.gap(), piece.text),
                None => piece.text.clone(),
            };
            if !out.is_empty() && self.width(style, &joined) <= room {
                if let Some(line) = out.last_mut() {
                    *line = joined;
                }
            } else if out.len() < lines {
                out.push(piece.text);
            } else if let Some(line) = out.last_mut() {
                // No line left: the rest goes onto the last one and is cut there.
                for rest in std::iter::once(piece).chain(pieces.by_ref()) {
                    line.push_str(rest.gap());
                    line.push_str(&rest.text);
                }
            }
        }
        out.into_iter().map(|line| self.fit(style, &line, room)).collect()
    }
}

/// A run of text that stays on one line.
struct Piece {
    text: String,
    /// A space stood before it in the text.
    spaced: bool,
}

impl Piece {
    /// What joins this piece to the one before it on the same line.
    fn gap(&self) -> &'static str {
        if self.spaced { " " } else { "" }
    }
}

/// The pieces a line may be broken between. A word of Latin letters is one
/// piece. Japanese and Chinese may break before and after any character,
/// except before closing punctuation, which never starts a line.
fn break_pieces(text: &str) -> Vec<Piece> {
    let mut pieces = Vec::new();
    for word in text.split_whitespace() {
        let mut spaced = true;
        let mut current = String::new();
        let mut after_wide = false;
        for ch in word.chars() {
            let wide = is_wide(ch);
            if !current.is_empty() && (wide || after_wide) && !closes_a_phrase(ch) {
                pieces.push(Piece { text: std::mem::take(&mut current), spaced });
                spaced = false;
            }
            current.push(ch);
            after_wide = wide;
        }
        if !current.is_empty() {
            pieces.push(Piece { text: current, spaced });
        }
    }
    pieces
}

/// Chinese characters, kana and their punctuation, and full-width forms.
fn is_wide(ch: char) -> bool {
    matches!(u32::from(ch), 0x2E80..=0x9FFF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF)
}

fn closes_a_phrase(ch: char) -> bool {
    matches!(ch, '、' | '。' | '，' | '．' | '：' | '；' | '！' | '？' | '）' | '」' | '』' | '】' | '…' | 'ー')
}

fn load_font(path: &Path) -> Option<fontdue::Font> {
    let bytes = std::fs::read(path).ok()?;
    fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()
}

/// File names of interface fonts, most wanted first: Segoe UI on Windows, and
/// the usual sans-serif families of Linux desktops.
const REGULAR_FILES: &[&str] = &[
    "segoeui.ttf",
    "NotoSans-Regular.ttf",
    "DejaVuSans.ttf",
    "Cantarell-Regular.otf",
    "Cantarell-VF.otf",
    "LiberationSans-Regular.ttf",
    "Ubuntu-R.ttf",
    "arial.ttf",
    "tahoma.ttf",
];
const BOLD_FILES: &[&str] = &[
    "seguisb.ttf",
    "segoeuib.ttf",
    "NotoSans-Bold.ttf",
    "DejaVuSans-Bold.ttf",
    "Cantarell-Bold.otf",
    "LiberationSans-Bold.ttf",
    "Ubuntu-B.ttf",
    "arialbd.ttf",
    "tahomabd.ttf",
];
/// The font files installed on this system, by lower-case file name.
struct FontFiles(HashMap<String, PathBuf>);

impl FontFiles {
    fn scan() -> Self {
        let mut files = HashMap::new();
        for dir in font_dirs() {
            collect_fonts(&dir, 4, &mut files);
        }
        Self(files)
    }

    fn first(&self, names: &[&str]) -> Option<PathBuf> {
        names.iter().find_map(|name| self.0.get(&name.to_ascii_lowercase()).cloned())
    }
}

fn font_dirs() -> Vec<PathBuf> {
    if cfg!(windows) {
        let windows = std::env::var_os("WINDIR").or_else(|| std::env::var_os("SystemRoot")).map_or_else(|| PathBuf::from("C:\\Windows"), PathBuf::from);
        return vec![windows.join("Fonts")];
    }
    let mut dirs = vec![PathBuf::from("/usr/share/fonts"), PathBuf::from("/usr/local/share/fonts")];
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        dirs.push(home.join(".local/share/fonts"));
        dirs.push(home.join(".fonts"));
    }
    dirs
}

/// Adds the font files under `dir`, down to `depth` folders deep. The first
/// file of a name wins, so a system font is not shadowed by a later folder.
fn collect_fonts(dir: &Path, depth: usize, files: &mut HashMap<String, PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth > 0 {
                collect_fonts(&path, depth - 1, files);
            }
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()).map(str::to_ascii_lowercase) else { continue };
        if [".ttf", ".otf", ".ttc"].iter().any(|ending| name.ends_with(ending)) {
            files.entry(name).or_insert(path);
        }
    }
}

/// Asks fontconfig for a font file. Linux only; elsewhere there is no `fc-match`.
fn fc_match(pattern: &str) -> Option<PathBuf> {
    if cfg!(windows) {
        return None;
    }
    let output = std::process::Command::new("fc-match").args(["-f", "%{file}", pattern]).output().ok()?;
    let path = PathBuf::from(String::from_utf8(output.stdout).ok()?.trim());
    path.is_file().then_some(path)
}

/// Draws onto the panel's pixel buffer.
struct Painter<'a> {
    pixmap: &'a mut Pixmap,
    fonts: &'a mut Fonts,
}

fn paint_of(color: Rgb, alpha: f32) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color.0, color.1, color.2, (alpha.clamp(0.0, 1.0) * 255.0).round() as u8);
    paint.anti_alias = true;
    paint
}

fn round_stroke(width: f32) -> Stroke {
    Stroke { width, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Stroke::default() }
}

/// A rectangle with quarter-circle corners.
fn rounded(rect: Rect, radius: f32) -> Option<tiny_skia::Path> {
    let Rect { x, y, w, h } = rect;
    let r = radius.min(w / 2.0).min(h / 2.0);
    // Distance of a cubic's control points that makes it a quarter circle.
    let k = r * 0.552_284_8;
    let mut path = PathBuilder::new();
    path.move_to(x + r, y);
    path.line_to(x + w - r, y);
    path.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    path.line_to(x + w, y + h - r);
    path.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    path.line_to(x + r, y + h);
    path.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    path.line_to(x, y + r);
    path.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    path.close();
    path.finish()
}

impl Painter<'_> {
    fn fill_rounded(&mut self, rect: Rect, radius: f32, color: Rgb) {
        if let Some(path) = rounded(rect, radius) {
            self.pixmap.fill_path(&path, &paint_of(color, 1.0), FillRule::Winding, Transform::identity(), None);
        }
    }

    fn stroke_rounded(&mut self, rect: Rect, radius: f32, color: Rgb, width: f32) {
        if let Some(path) = rounded(rect, radius) {
            self.pixmap.stroke_path(&path, &paint_of(color, 1.0), &round_stroke(width), Transform::identity(), None);
        }
    }

    fn circle(&mut self, cx: f32, cy: f32, radius: f32, color: Rgb, alpha: f32) {
        if let Some(path) = PathBuilder::from_circle(cx, cy, radius) {
            self.pixmap.fill_path(&path, &paint_of(color, alpha), FillRule::Winding, Transform::identity(), None);
        }
    }

    /// Draws one line of text with its baseline at `baseline` and returns its width.
    fn text(&mut self, style: Style, x: f32, baseline: f32, text: &str) -> f32 {
        let Style { face, px, color } = style;
        let mut pen = x;
        for ch in text.chars() {
            let face = self.fonts.face_for(face, ch);
            let key = (face, ch, px.to_bits());
            if !self.fonts.glyphs.contains_key(&key) {
                let Some(font) = self.fonts.font(face) else { continue };
                let glyph = font.rasterize(ch, px);
                self.fonts.glyphs.insert(key, glyph);
            }
            let Some((metrics, coverage)) = self.fonts.glyphs.get(&key) else { continue };
            let left = (pen + metrics.xmin as f32).round() as i32;
            let top = (baseline - metrics.ymin as f32 - metrics.height as f32).round() as i32;
            blend_glyph(self.pixmap, coverage, metrics.width, left, top, color);
            pen += metrics.advance_width;
        }
        pen - x
    }

    /// Draws `text` cut to `room` pixels.
    fn text_fit(&mut self, style: Style, x: f32, baseline: f32, room: f32, text: &str) {
        let fitted = self.fonts.fit(style, text, room);
        self.text(style, x, baseline, &fitted);
    }

    /// Draws `text` on up to two lines, `leading` pixels apart.
    fn text_wrapped(&mut self, style: Style, x: f32, baseline: f32, room: f32, leading: f32, text: &str) {
        for (line, words) in self.fonts.wrap(style, text, room, 2).into_iter().enumerate() {
            self.text(style, x, baseline + line as f32 * leading, &words);
        }
    }
}

/// Lays a glyph's coverage over the picture in `color`.
fn blend_glyph(pixmap: &mut Pixmap, coverage: &[u8], glyph_width: usize, left: i32, top: i32, color: Rgb) {
    if glyph_width == 0 {
        return;
    }
    let (width, height) = (pixmap.width() as i32, pixmap.height() as i32);
    let data = pixmap.data_mut();
    for (row, line) in coverage.chunks_exact(glyph_width).enumerate() {
        let y = top + row as i32;
        if y < 0 || y >= height {
            continue;
        }
        for (column, cover) in line.iter().enumerate() {
            let x = left + column as i32;
            if *cover == 0 || x < 0 || x >= width {
                continue;
            }
            let at = ((y * width + x) * 4) as usize;
            let alpha = u32::from(*cover);
            let over = |source: u8, below: u8| ((u32::from(source) * alpha + u32::from(below) * (255 - alpha) + 127) / 255) as u8;
            data[at] = over(color.0, data[at]);
            data[at + 1] = over(color.1, data[at + 1]);
            data[at + 2] = over(color.2, data[at + 2]);
            data[at + 3] = over(255, data[at + 3]);
        }
    }
}

// The PC glyph of the headset mixer's PC streaming output, so a stream looks
// the same at both ends: andrsrxn:device-pc-speaker, view box 24, MIT,
// Andrés Raxón. The speaker box, its cone and its dot are drawn as shapes.
const PC_SCREEN_FILL: &str = "M1.66 10.14c0-1.89 0-2.83.58-3.42c.59-.58 1.53-.58 3.42-.58H8c1.89 0 2.83 0 3.42.58c.58.59.58 1.53.58 3.42v2.25c0 1.88 0 2.83-.58 3.41c-.59.59-1.53.59-3.42.59H5.66c-1.89 0-2.83 0-3.42-.59c-.58-.58-.58-1.53-.58-3.41z";
const PC_SCREEN_STROKE: &str = "M12 6.14H5.66c-1.89 0-2.83 0-3.42.58c-.58.59-.58 1.53-.58 3.42v2.25c0 1.88 0 2.83.58 3.41c.59.59 1.53.59 3.42.59h6.1m-5.99 3.07h4.31m-2.15 0v-2.98";
const PC_VIEW: f32 = 24.0;

/// Reads the subset of SVG path data the glyph uses: M, L, H, V, C and Z,
/// absolute or relative, with numbers packed the way icon sets pack them.
fn svg_path(data: &str) -> Option<tiny_skia::Path> {
    let mut path = PathBuilder::new();
    let mut numbers = SvgNumbers { rest: data };
    let (mut x, mut y, mut start) = (0.0f32, 0.0f32, (0.0f32, 0.0f32));
    let mut command = None;
    loop {
        numbers.skip_separators();
        if let Some(letter) = numbers.rest.chars().next().filter(char::is_ascii_alphabetic) {
            numbers.rest = &numbers.rest[1..];
            command = Some(letter);
            if letter.eq_ignore_ascii_case(&'z') {
                path.close();
                (x, y) = start;
                continue;
            }
        } else if numbers.rest.is_empty() {
            break;
        }
        let letter = command?;
        let relative = letter.is_ascii_lowercase();
        let (base_x, base_y) = if relative { (x, y) } else { (0.0, 0.0) };
        match letter.to_ascii_uppercase() {
            'M' => {
                (x, y) = (base_x + numbers.next()?, base_y + numbers.next()?);
                path.move_to(x, y);
                start = (x, y);
                // Further pairs after a move are lines.
                command = Some(if relative { 'l' } else { 'L' });
            }
            'L' => {
                (x, y) = (base_x + numbers.next()?, base_y + numbers.next()?);
                path.line_to(x, y);
            }
            'H' => {
                x = base_x + numbers.next()?;
                path.line_to(x, y);
            }
            'V' => {
                y = base_y + numbers.next()?;
                path.line_to(x, y);
            }
            'C' => {
                let (x1, y1) = (base_x + numbers.next()?, base_y + numbers.next()?);
                let (x2, y2) = (base_x + numbers.next()?, base_y + numbers.next()?);
                (x, y) = (base_x + numbers.next()?, base_y + numbers.next()?);
                path.cubic_to(x1, y1, x2, y2, x, y);
            }
            _ => return None,
        }
    }
    path.finish()
}

struct SvgNumbers<'a> {
    rest: &'a str,
}

impl SvgNumbers<'_> {
    fn skip_separators(&mut self) {
        self.rest = self.rest.trim_start_matches(|ch: char| ch.is_whitespace() || ch == ',');
    }

    /// The next number. A second dot or a sign ends one number and starts the
    /// next, so "-.58.59-3" is three numbers.
    fn next(&mut self) -> Option<f32> {
        self.skip_separators();
        let mut end = 0;
        let mut seen_dot = false;
        for (index, ch) in self.rest.char_indices() {
            let part_of_number = match ch {
                '0'..='9' => true,
                '-' | '+' => index == 0,
                '.' if !seen_dot => {
                    seen_dot = true;
                    true
                }
                _ => false,
            };
            if !part_of_number {
                break;
            }
            end = index + ch.len_utf8();
        }
        let value = self.rest[..end].parse().ok()?;
        self.rest = &self.rest[end..];
        Some(value)
    }
}

/// The PC glyph in a `size` square at (`x`, `y`). `crossed_over` strikes it
/// through, with a gap in that color so the line stands clear of the glyph.
fn pc_icon(pixmap: &mut Pixmap, x: f32, y: f32, size: f32, color: Rgb, crossed_over: Option<Rgb>) {
    let transform = Transform::from_scale(size / PC_VIEW, size / PC_VIEW).post_translate(x, y);
    let soft = paint_of(color, 0.2);
    let solid = paint_of(color, 1.0);
    let line = round_stroke(1.5);
    if let Some(screen) = svg_path(PC_SCREEN_FILL) {
        pixmap.fill_path(&screen, &soft, FillRule::Winding, transform, None);
    }
    if let Some(cone) = PathBuilder::from_circle(17.17, 14.12, 2.34) {
        pixmap.fill_path(&cone, &soft, FillRule::Winding, transform, None);
    }
    if let Some(outline) = svg_path(PC_SCREEN_STROKE) {
        pixmap.stroke_path(&outline, &solid, &line, transform, None);
    }
    if let Some(speaker) = rounded(Rect { x: 12.0, y: 4.54, w: 10.34, h: 14.92 }, 2.0) {
        pixmap.stroke_path(&speaker, &solid, &line, transform, None);
    }
    if let Some(cone) = PathBuilder::from_circle(17.17, 14.12, 2.17) {
        pixmap.stroke_path(&cone, &solid, &line, transform, None);
    }
    if let Some(dot) = PathBuilder::from_circle(17.17, 8.1, 0.9) {
        pixmap.fill_path(&dot, &solid, FillRule::Winding, transform, None);
    }
    if let Some(backdrop) = crossed_over {
        let mut slash = PathBuilder::new();
        slash.move_to(3.0, 3.0);
        slash.line_to(21.0, 21.0);
        if let Some(slash) = slash.finish() {
            pixmap.stroke_path(&slash, &paint_of(backdrop, 1.0), &round_stroke(4.5), transform, None);
            pixmap.stroke_path(&slash, &solid, &round_stroke(1.7), transform, None);
        }
    }
}

fn draw(painter: &mut Painter<'_>, view: &View) {
    let text = view.lang.text();
    painter.pixmap.fill(tiny_skia::Color::from_rgba8(BG.0, BG.1, BG.2, 255));
    draw_header(painter, view, text);
    for (kind, tiles) in KINDS.into_iter().zip(&view.tiles) {
        let (title, note) = match kind {
            Kind::Mic => (text.mics, text.mics_note),
            Kind::Output => (text.outputs, text.outputs_note),
        };
        let baseline = GROUP_TOP[group_of(kind)] + 26.0;
        let title_w = painter.text(bold(23.0, TEXT), MARGIN, baseline, title);
        let note_x = MARGIN + title_w + 18.0;
        painter.text_fit(regular(18.0, MUTED), note_x, baseline, WIDTH as f32 - MARGIN - note_x, note);
        for (index, tile) in tiles.iter().enumerate() {
            draw_tile(painter, tile_rect(kind, index), tile, text);
        }
    }
    // Last, so it lies over the tiles.
    if let Some(menu) = &view.menu {
        draw_menu(painter, menu, view.lang);
    }
}

/// How a button or a menu row answers the pointer: `base` a little lighter.
fn touched(base: Rgb, touch: Touch) -> Rgb {
    match touch {
        Touch::None => base,
        Touch::Hover => mix(WHITE, base, 0.07),
        Touch::Pressed => mix(WHITE, base, 0.14),
    }
}

/// The language button: a globe, the language's own name, and an arrow that
/// points up while the menu is open.
fn draw_language_button(painter: &mut Painter<'_>, view: &View) {
    let Rect { x, y, w, h } = LANG_BUTTON;
    let open = view.menu.is_some();
    let fill = if open { mix(ACCENT, INSET, 0.16) } else { touched(INSET, view.language_button) };
    painter.fill_rounded(LANG_BUTTON, h / 2.0, fill);

    let (cx, cy, radius) = (x + 24.0, y + h / 2.0, 9.5);
    let line = round_stroke(1.6);
    let ink = paint_of(MUTED, 1.0);
    let mut globe = PathBuilder::new();
    globe.push_circle(cx, cy, radius);
    if let Some(meridian) = tiny_skia::Rect::from_xywh(cx - 4.2, cy - radius, 8.4, 2.0 * radius) {
        globe.push_oval(meridian);
    }
    globe.move_to(cx - radius, cy);
    globe.line_to(cx + radius, cy);
    if let Some(globe) = globe.finish() {
        painter.pixmap.stroke_path(&globe, &ink, &line, Transform::identity(), None);
    }

    let (ax, ay) = (x + w - 22.0, cy);
    let tip = if open { -3.0 } else { 3.0 };
    let mut arrow = PathBuilder::new();
    arrow.move_to(ax - 5.5, ay - tip);
    arrow.line_to(ax, ay + tip);
    arrow.line_to(ax + 5.5, ay - tip);
    if let Some(arrow) = arrow.finish() {
        painter.pixmap.stroke_path(&arrow, &ink, &round_stroke(1.8), Transform::identity(), None);
    }

    let name_x = x + 42.0;
    painter.text_fit(regular(18.0, TEXT), name_x, cy + 6.5, ax - 12.0 - name_x, view.lang.name());
}

/// The open language menu: one row per language, each in its own language.
fn draw_menu(painter: &mut Painter<'_>, menu: &MenuView, current: Lang) {
    let frame = menu_rect();
    // A soft dark edge lifts the menu off the tiles it covers.
    let shade = Rect { x: frame.x - 6.0, y: frame.y - 4.0, w: frame.w + 12.0, h: frame.h + 14.0 };
    if let Some(path) = rounded(shade, 22.0) {
        painter.pixmap.fill_path(&path, &paint_of(BG, 0.6), FillRule::Winding, Transform::identity(), None);
    }
    painter.fill_rounded(frame, 16.0, CARD);
    painter.stroke_rounded(frame, 16.0, mix(WHITE, CARD, 0.16), 1.5);
    for (row, lang) in Lang::ALL.into_iter().enumerate() {
        let rect = menu_row_rect(row);
        let touch = menu.touched.filter(|(at, _)| *at == row).map_or(Touch::None, |(_, touch)| touch);
        let chosen = lang == current;
        if chosen || touch != Touch::None {
            let base = if chosen { mix(ACCENT, CARD, 0.14) } else { CARD };
            painter.fill_rounded(rect, 10.0, touched(base, touch));
        }
        let middle = rect.y + rect.h / 2.0;
        if chosen {
            painter.circle(rect.x + 18.0, middle, 4.5, ACCENT, 1.0);
        }
        let ink = if chosen { ACCENT } else { TEXT };
        painter.text_fit(regular(20.0, ink), rect.x + 36.0, middle + 7.0, rect.w - 48.0, lang.name());
    }
}

fn draw_header(painter: &mut Painter<'_>, view: &View, text: &Text) {
    let status = &view.status;
    let logo = Rect { x: MARGIN, y: 24.0, w: 80.0, h: 80.0 };
    painter.fill_rounded(logo, 20.0, mix(ACCENT, CARD, 0.14));
    pc_icon(painter.pixmap, logo.x + 14.0, logo.y + 14.0, 52.0, ACCENT, None);

    let box_x = WIDTH as f32 - MARGIN - STATUS_W;
    let text_x = logo.x + logo.w + 20.0;
    painter.text_fit(bold(36.0, TEXT), text_x, 62.0, LANG_BUTTON.x - 16.0 - text_x, text.title);
    painter.text_wrapped(regular(18.0, MUTED), text_x, 88.0, box_x - 24.0 - text_x, 22.0, text.hint);
    draw_language_button(painter, view);

    let frame = Rect { x: box_x, y: 16.0, w: STATUS_W, h: STATUS_H };
    painter.fill_rounded(frame, 18.0, CARD);
    let color = status.tone.color();
    painter.circle(frame.x + 28.0, frame.y + 29.0, 12.0, color, 0.22);
    painter.circle(frame.x + 28.0, frame.y + 29.0, 6.5, color, 1.0);
    painter.text_fit(bold(22.0, TEXT), frame.x + 50.0, frame.y + 37.0, frame.w - 70.0, &status.title);
    painter.text_wrapped(regular(16.0, MUTED), frame.x + 20.0, frame.y + 62.0, frame.w - 40.0, 20.0, &status.detail);
}

fn draw_tile(painter: &mut Painter<'_>, rect: Rect, tile: &TileView, text: &Text) {
    let Rect { x, y, w, h } = rect;
    let base = if tile.on { mix(ACCENT, INSET, 0.12) } else { INSET };
    let fill = match tile.touch {
        Touch::None => base,
        Touch::Hover => mix(WHITE, base, 0.07),
        Touch::Pressed => mix(WHITE, base, 0.14),
    };
    painter.fill_rounded(rect, TILE_RADIUS, fill);
    if tile.on {
        let edge = Rect { x: x + 1.0, y: y + 1.0, w: w - 2.0, h: h - 2.0 };
        painter.stroke_rounded(edge, TILE_RADIUS - 1.0, mix(ACCENT, INSET, 0.5), 2.0);
    }

    // The stream's label, with the activity light beside it.
    let ink = if tile.on { TEXT } else { MUTED };
    let label_w = painter.text(bold(44.0, ink), x + TILE_PAD, y + 60.0, &tile.label);
    let light = (x + TILE_PAD + label_w + 22.0, y + 44.0);
    if tile.live {
        painter.circle(light.0, light.1, 14.0, GOOD, 0.25);
        painter.circle(light.0, light.1, 8.0, GOOD, 1.0);
    } else {
        painter.circle(light.0, light.1, 8.0, CONTROL, 1.0);
    }

    // The same glyph as in the headset mixer: plain while the stream is on, struck through while it is off.
    let icon = 46.0;
    let (icon_color, crossed) = if tile.on { (ACCENT, None) } else { (MUTED, Some(fill)) };
    pc_icon(painter.pixmap, x + w - TILE_PAD - icon, y + 20.0, icon, icon_color, crossed);

    let room = w - 2.0 * TILE_PAD;
    // "Steam Streaming Out PC1" is a little too wide at full size, and the
    // part an ellipsis would cut is the number that tells the devices apart.
    let name = if painter.fonts.width(regular(21.0, ink), &tile.name) <= room { regular(21.0, ink) } else { regular(19.0, ink) };
    painter.text_fit(name, x + TILE_PAD, y + 100.0, room, &tile.name);
    painter.text_wrapped(regular(16.5, tile.tone.color()), x + TILE_PAD, y + 128.0, room, 21.0, tile.note);

    // The switch: a track with a knob, and the word for its position.
    let track = Rect { x: x + TILE_PAD, y: y + h - TILE_PAD - 44.0, w: 84.0, h: 44.0 };
    let knob_y = track.y + track.h / 2.0;
    if tile.on {
        painter.fill_rounded(track, 22.0, ACCENT);
        painter.circle(track.x + track.w - 22.0, knob_y, 16.0, ACCENT_INK, 1.0);
    } else {
        painter.fill_rounded(track, 22.0, CONTROL);
        painter.circle(track.x + 22.0, knob_y, 16.0, MUTED, 1.0);
    }
    let word = if tile.on { text.on } else { text.off };
    painter.text(bold(23.0, ink), track.x + track.w + 16.0, knob_y + 8.0, word);
}

/// The dashboard panel: remembers what is on screen and where the pointer is.
pub struct Panel {
    lang: Lang,
    fonts: Fonts,
    /// `None` only if the buffer could not be allocated; nothing is drawn then.
    pixmap: Option<Pixmap>,
    shown: Option<View>,
    pointer: Pointer,
    menu_open: bool,
}

impl Panel {
    #[must_use]
    pub fn in_language(lang: Lang) -> Self {
        Self { lang, fonts: Fonts::load(lang), pixmap: Pixmap::new(WIDTH, HEIGHT), shown: None, pointer: Pointer::default(), menu_open: false }
    }

    /// Forgets what is on screen, so the next `render` draws again. Needed
    /// after a new session with SteamVR, which starts with an empty overlay.
    pub fn invalidate(&mut self) {
        self.shown = None;
    }

    /// The pointer moved to (`x`, `y`), in panel pixels from the top left.
    pub fn pointer_move(&mut self, x: f32, y: f32) {
        self.pointer.hover = target_at(x, y, self.menu_open);
    }

    /// The pointer left the panel.
    pub fn pointer_leave(&mut self) {
        self.pointer = Pointer::default();
    }

    pub fn pointer_down(&mut self, x: f32, y: f32) {
        self.pointer.hover = target_at(x, y, self.menu_open);
        self.pointer.pressed = self.pointer.hover;
        // A press beside the open menu closes it and does nothing else.
        if self.menu_open && self.pointer.hover.is_none() {
            self.menu_open = false;
            self.pointer.hover = target_at(x, y, false);
        }
    }

    /// The press ended. It counts when it ends on what it began on, so a
    /// laser pointer that slips off a tile does not switch its neighbour.
    pub fn pointer_up(&mut self, x: f32, y: f32) -> Option<Action> {
        let pressed = self.pointer.pressed.take();
        let released_on = target_at(x, y, self.menu_open);
        let action = match pressed.filter(|pressed| released_on == Some(*pressed)) {
            Some(Target::Tile(kind, index)) => Some(Action::Toggle(kind, index)),
            Some(Target::LanguageButton) => {
                self.menu_open = !self.menu_open;
                None
            }
            Some(Target::Language(row)) => {
                self.menu_open = false;
                Lang::ALL.get(row).copied().map(|lang| {
                    self.set_language(lang);
                    Action::Language(lang.code())
                })
            }
            None => None,
        };
        // The menu opened or closed under the pointer: what lies there changed.
        self.pointer.hover = target_at(x, y, self.menu_open);
        action
    }

    fn set_language(&mut self, lang: Lang) {
        self.lang = lang;
        self.fonts.set_language(lang);
    }

    /// The panel's pixels (RGBA, top row first) when the picture differs from
    /// the one returned last, otherwise `None`.
    pub fn render(&mut self, snap: &Snapshot) -> Option<&[u8]> {
        let next = view(snap, self.lang, self.pointer, self.menu_open);
        if self.shown.as_ref() == Some(&next) {
            return None;
        }
        let pixmap = self.pixmap.as_mut()?;
        draw(&mut Painter { pixmap, fonts: &mut self.fonts }, &next);
        self.shown = Some(next);
        // The panel is opaque, so premultiplied and plain RGBA are the same bytes.
        self.pixmap.as_ref().map(Pixmap::data)
    }
}

fn thumbnail_pixmap() -> Option<Pixmap> {
    let mut pixmap = Pixmap::new(THUMBNAIL, THUMBNAIL)?;
    let edge = THUMBNAIL as f32;
    if let Some(plate) = rounded(Rect { x: 0.0, y: 0.0, w: edge, h: edge }, 56.0) {
        pixmap.fill_path(&plate, &paint_of(CARD, 1.0), FillRule::Winding, Transform::identity(), None);
    }
    pc_icon(&mut pixmap, 36.0, 36.0, edge - 72.0, ACCENT, None);
    Some(pixmap)
}

/// The dashboard icon as plain (not premultiplied) RGBA, `THUMBNAIL` pixels square.
#[must_use]
pub fn thumbnail() -> Vec<u8> {
    let Some(pixmap) = thumbnail_pixmap() else { return Vec::new() };
    pixmap.pixels().iter().map(|pixel| pixel.demultiply()).flat_map(|color| [color.red(), color.green(), color.blue(), color.alpha()]).collect()
}

/// Example states of the panel, by name.
fn samples() -> Vec<(&'static str, Snapshot)> {
    use frame_link_core::LinkStatus;
    use DeviceState::{Failed, Missing, Ready};
    let ready = crate::model::AudioStatus { mics: [Ready; STREAMS], outputs: [Ready; STREAMS] };
    let connected = LinkStatus {
        state: LinkState::Connected,
        peer: "Steam Frame".into(),
        peer_addr: "10.35.78.1".into(),
        rtt_ms: 2.4,
        remote_send: 0b1111,
        remote_recv: 0b0011,
        receiving: 0b0001,
        sending: 0b0010,
        names: vec!["Discord".into(), "OBS commentary".into(), String::new(), "配信マイク".into()],
        ..LinkStatus::default()
    };
    let refused = |reason: &str| Snapshot {
        link: LinkStatus { state: LinkState::Refused, refused: reason.into(), ..LinkStatus::default() },
        audio: ready.clone(),
        mics_on: [true; STREAMS],
        outputs_on: [true; STREAMS],
    };
    vec![
        ("searching", Snapshot { link: LinkStatus::default(), audio: ready.clone(), mics_on: [true; STREAMS], outputs_on: [true; STREAMS] }),
        (
            "connected",
            Snapshot {
                link: connected.clone(),
                audio: crate::model::AudioStatus { mics: [Ready; STREAMS], outputs: [Ready, Ready, Ready, Failed] },
                mics_on: [true, true, false, true],
                outputs_on: [true, true, true, true],
            },
        ),
        (
            "missing",
            Snapshot { link: connected, audio: crate::model::AudioStatus { mics: [Missing; STREAMS], outputs: [Missing; STREAMS] }, mics_on: [true; STREAMS], outputs_on: [true, true, false, false] },
        ),
        ("refused-busy", refused("busy")),
        ("refused-version", refused("version")),
        ("refused-not-allowed", refused("not-allowed")),
    ]
}

/// Writes the panel in its main states and every language, and the dashboard
/// icon, as PNG files into `dir`. For looking at the overlay without a headset.
///
/// # Errors
/// When a file cannot be written.
pub fn write_previews(dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    for lang in Lang::ALL {
        let mut panel = Panel::in_language(lang);
        let save = |panel: &Panel, name: &str| -> anyhow::Result<()> {
            let path = dir.join(format!("panel-{name}-{}.png", lang.code()));
            let pixmap = panel.pixmap.as_ref().context("the panel has no pixel buffer")?;
            pixmap.save_png(&path).with_context(|| format!("writing {}", path.display()))?;
            println!("{}", path.display());
            Ok(())
        };
        for (name, snap) in samples() {
            if name == "connected" {
                // One tile under the pointer, to see the hover state as well.
                let hovered = tile_rect(Kind::Mic, 1);
                panel.pointer_move(hovered.x + 10.0, hovered.y + 10.0);
            } else {
                panel.pointer_leave();
            }
            panel.invalidate();
            let _ = panel.render(&snap);
            save(&panel, name)?;
        }
        // The language menu, open, with the pointer on one of its rows.
        if let Some((_, snap)) = samples().into_iter().find(|(name, _)| *name == "connected") {
            let (x, y) = (LANG_BUTTON.x + LANG_BUTTON.w / 2.0, LANG_BUTTON.y + LANG_BUTTON.h / 2.0);
            panel.pointer_down(x, y);
            let _ = panel.pointer_up(x, y);
            let row = menu_row_rect(2);
            panel.pointer_move(row.x + row.w / 2.0, row.y + row.h / 2.0);
            let _ = panel.render(&snap);
            save(&panel, "menu")?;
        }
    }
    let path = dir.join("thumbnail.png");
    thumbnail_pixmap().context("the icon has no pixel buffer")?.save_png(&path).with_context(|| format!("writing {}", path.display()))?;
    println!("{}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AudioStatus;
    use frame_link_core::LinkStatus;

    fn center(kind: Kind, index: usize) -> (f32, f32) {
        let rect = tile_rect(kind, index);
        (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0)
    }

    /// Connected, every device open, every stream on and wanted by the headset.
    fn healthy() -> Snapshot {
        Snapshot {
            link: LinkStatus { state: LinkState::Connected, remote_send: 0b1111, remote_recv: 0b1111, ..LinkStatus::default() },
            audio: AudioStatus { mics: [DeviceState::Ready; STREAMS], outputs: [DeviceState::Ready; STREAMS] },
            mics_on: [true; STREAMS],
            outputs_on: [true; STREAMS],
        }
    }

    fn note_of(snap: &Snapshot, kind: Kind, index: usize) -> &'static str {
        tile_view(snap, &EN, kind, index, Touch::None).note
    }

    fn press(panel: &mut Panel, (x, y): (f32, f32)) -> Option<Action> {
        panel.pointer_down(x, y);
        panel.pointer_up(x, y)
    }

    fn middle(rect: Rect) -> (f32, f32) {
        (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0)
    }

    /// True when `words` does not fit `lines` lines of `room` pixels whole.
    fn is_cut(fonts: &mut Fonts, style: Style, room: f32, lines: usize, words: &str) -> bool {
        let solid = |text: &str| text.chars().filter(|ch| !ch.is_whitespace()).collect::<String>();
        solid(&fonts.wrap(style, words, room, lines).concat()) != solid(words)
    }

    #[test]
    fn german_system_should_get_german_text() {
        assert_eq!(Lang::from_locale("de-AT"), Lang::De);
    }

    #[test]
    fn system_in_a_language_the_panel_lacks_should_get_english_text() {
        assert_eq!(Lang::from_locale("pt-BR"), Lang::En);
    }

    #[test]
    fn every_locale_form_should_find_its_language() {
        let found = ["ja_JP.UTF-8", "zh-CN", "fr-FR", "es_ES", "IT", "en-US"].map(Lang::from_locale);

        assert_eq!(found, [Lang::Ja, Lang::Sc, Lang::Fr, Lang::Es, Lang::It, Lang::En]);
    }

    #[test]
    fn saved_language_should_win_over_the_system() {
        assert_eq!(Lang::from_setting("ja"), Lang::Ja);
    }

    #[test]
    fn every_language_should_load_back_from_its_code() {
        assert_eq!(Lang::ALL.map(|lang| Lang::from_setting(lang.code())), Lang::ALL);
    }

    #[test]
    fn every_language_should_keep_the_placeholders() {
        let broken: Vec<&str> = Lang::ALL
            .into_iter()
            .filter(|lang| {
                let text = lang.text();
                !text.connected.contains("{peer}") || !text.connected_detail.contains("{addr}") || !text.connected_detail.contains("{rtt}")
            })
            .map(Lang::code)
            .collect();

        assert_eq!(broken, Vec::<&str>::new());
    }

    #[test]
    fn language_button_should_open_the_menu() {
        let mut panel = Panel::in_language(Lang::En);

        let action = press(&mut panel, middle(LANG_BUTTON));

        assert!(action.is_none() && panel.menu_open);
    }

    #[test]
    fn menu_row_should_switch_the_language_and_report_it() {
        let mut panel = Panel::in_language(Lang::En);
        press(&mut panel, middle(LANG_BUTTON));

        let action = press(&mut panel, middle(menu_row_rect(3)));

        assert_eq!((action, panel.lang, panel.menu_open), (Some(Action::Language("de")), Lang::De, false));
    }

    #[test]
    fn press_beside_the_open_menu_should_only_close_it() {
        let mut panel = Panel::in_language(Lang::En);
        press(&mut panel, middle(LANG_BUTTON));

        let action = press(&mut panel, center(Kind::Output, 0));

        assert!(action.is_none() && !panel.menu_open);
    }

    #[test]
    fn open_menu_should_cover_the_tile_below_it() {
        let (x, y) = middle(menu_row_rect(4));

        assert_eq!((tile_at(x, y).is_some(), target_at(x, y, true)), (true, Some(Target::Language(4))));
    }

    #[test]
    fn menu_should_stay_inside_the_panel() {
        let menu = menu_rect();

        assert!(menu.x >= 0.0 && menu.x + menu.w <= WIDTH as f32 && menu.y + menu.h <= HEIGHT as f32);
    }

    #[test]
    fn new_language_should_draw_a_new_picture() {
        let mut panel = Panel::in_language(Lang::En);
        let snap = healthy();
        let _ = panel.render(&snap);
        press(&mut panel, middle(LANG_BUTTON));
        press(&mut panel, middle(menu_row_rect(1)));
        panel.pointer_leave();

        assert!(panel.render(&snap).is_some());
    }

    #[test]
    fn chinese_should_break_between_characters() {
        let pieces: Vec<String> = break_pieces("头显 PC").into_iter().map(|piece| piece.text).collect();

        assert_eq!(pieces, ["头", "显", "PC"]);
    }

    #[test]
    fn closing_punctuation_should_not_start_a_line() {
        let pieces: Vec<String> = break_pieces("PC1–PC4，作为").into_iter().map(|piece| piece.text).collect();

        assert_eq!(pieces, ["PC1–PC4，", "作", "为"]);
    }

    #[test]
    fn latin_words_should_stay_whole() {
        let pieces: Vec<(String, bool)> = break_pieces("Sound device").into_iter().map(|piece| (piece.text, piece.spaced)).collect();

        assert_eq!(pieces, [("Sound".to_string(), true), ("device".to_string(), true)]);
    }

    #[test]
    fn text_should_fit_its_place_in_every_language() {
        let mut fonts = Fonts::load(Lang::En);
        // Nothing to measure with on a system without an interface font.
        if fonts.regular.is_none() {
            return;
        }
        let tile = tile_rect(Kind::Mic, 0).w - 2.0 * TILE_PAD;
        let hint_room = WIDTH as f32 - MARGIN - STATUS_W - 24.0 - (MARGIN + 100.0);
        let group_room = WIDTH as f32 - 2.0 * MARGIN - 18.0;
        let mut cut = Vec::new();
        for lang in Lang::ALL {
            fonts.set_language(lang);
            let text = lang.text();
            let note = regular(16.5, MUTED);
            let notes = [
                text.device_missing,
                text.device_failed,
                text.switched_off,
                text.waiting,
                text.not_offered,
                text.not_in_mixer,
                text.receiving,
                text.sending,
                text.ready,
            ];
            let details = [text.searching_detail, text.busy_detail, text.version_detail, text.not_allowed_detail];
            let titles = [text.searching, text.busy, text.version, text.not_allowed, text.refused];
            let mut places: Vec<(Style, f32, usize, &str)> = Vec::new();
            places.extend(notes.map(|words| (note, tile, 2, words)));
            places.extend(details.map(|words| (regular(16.0, MUTED), STATUS_W - 40.0, 2, words)));
            places.extend(titles.map(|words| (bold(22.0, TEXT), STATUS_W - 70.0, 1, words)));
            places.extend([text.on, text.off].map(|words| (bold(23.0, TEXT), tile - 84.0 - 16.0, 1, words)));
            places.push((regular(18.0, MUTED), hint_room, 2, text.hint));
            for (title, words) in [(text.mics, text.mics_note), (text.outputs, text.outputs_note)] {
                let title_w = fonts.width(bold(23.0, TEXT), title);
                places.push((regular(18.0, MUTED), group_room - title_w, 1, words));
            }
            places.push((regular(18.0, TEXT), LANG_BUTTON.w - 42.0 - 34.0, 1, lang.name()));
            for (style, room, lines, words) in places {
                if is_cut(&mut fonts, style, room, lines, words) {
                    cut.push(format!("{}: {words}", lang.code()));
                }
            }
        }

        assert_eq!(cut, Vec::<String>::new());
    }

    #[test]
    fn tile_center_should_hit_its_stream() {
        let (x, y) = center(Kind::Output, 2);

        assert_eq!(tile_at(x, y), Some((Kind::Output, 2)));
    }

    #[test]
    fn gap_between_tiles_should_hit_nothing() {
        let first = tile_rect(Kind::Mic, 0);

        assert_eq!(tile_at(first.x + first.w + TILE_GAP / 2.0, first.y + 10.0), None);
    }

    #[test]
    fn header_should_hit_nothing() {
        assert_eq!(tile_at(200.0, 60.0), None);
    }

    #[test]
    fn tiles_should_stay_inside_the_panel() {
        let last = tile_rect(Kind::Output, STREAMS - 1);

        assert!(last.x + last.w <= WIDTH as f32 - MARGIN + 0.01 && last.y + last.h <= HEIGHT as f32);
    }

    #[test]
    fn press_and_release_on_a_tile_should_toggle_it() {
        let mut panel = Panel::in_language(Lang::En);
        let (x, y) = center(Kind::Mic, 3);
        panel.pointer_down(x, y);

        assert_eq!(panel.pointer_up(x, y), Some(Action::Toggle(Kind::Mic, 3)));
    }

    #[test]
    fn release_on_another_tile_should_toggle_nothing() {
        let mut panel = Panel::in_language(Lang::En);
        let (x, y) = center(Kind::Mic, 0);
        panel.pointer_down(x, y);
        let (x, y) = center(Kind::Mic, 1);

        assert_eq!(panel.pointer_up(x, y), None);
    }

    #[test]
    fn release_without_a_press_should_toggle_nothing() {
        let mut panel = Panel::in_language(Lang::En);
        let (x, y) = center(Kind::Output, 0);

        assert_eq!(panel.pointer_up(x, y), None);
    }

    #[test]
    fn press_that_leaves_the_panel_should_be_forgotten() {
        let mut panel = Panel::in_language(Lang::En);
        let (x, y) = center(Kind::Output, 0);
        panel.pointer_down(x, y);
        panel.pointer_leave();

        assert_eq!(panel.pointer_up(x, y), None);
    }

    #[test]
    fn unchanged_picture_should_not_be_drawn_twice() {
        let mut panel = Panel::in_language(Lang::En);
        let snap = healthy();
        let _ = panel.render(&snap);

        assert!(panel.render(&snap).is_none());
    }

    #[test]
    fn first_picture_should_cover_the_whole_panel() {
        let mut panel = Panel::in_language(Lang::En);

        assert_eq!(panel.render(&healthy()).map(<[u8]>::len), Some((WIDTH * HEIGHT * 4) as usize));
    }

    #[test]
    fn hover_should_draw_a_new_picture() {
        let mut panel = Panel::in_language(Lang::En);
        let snap = healthy();
        let _ = panel.render(&snap);
        let (x, y) = center(Kind::Mic, 0);
        panel.pointer_move(x, y);

        assert!(panel.render(&snap).is_some());
    }

    #[test]
    fn invalidated_panel_should_draw_again() {
        let mut panel = Panel::in_language(Lang::En);
        let snap = healthy();
        let _ = panel.render(&snap);
        panel.invalidate();

        assert!(panel.render(&snap).is_some());
    }

    #[test]
    fn missing_device_should_be_named_before_anything_else() {
        let mut snap = healthy();
        snap.audio.mics[1] = DeviceState::Missing;
        snap.mics_on[1] = false;

        assert_eq!(note_of(&snap, Kind::Mic, 1), EN.device_missing);
    }

    #[test]
    fn switched_off_stream_should_say_it_uses_no_bandwidth() {
        let mut snap = healthy();
        snap.outputs_on[0] = false;

        assert_eq!(note_of(&snap, Kind::Output, 0), EN.switched_off);
    }

    #[test]
    fn output_without_a_mixer_channel_should_say_nothing_is_sent() {
        let mut snap = healthy();
        snap.link.remote_recv = 0b1110;

        assert_eq!(note_of(&snap, Kind::Output, 0), EN.not_in_mixer);
    }

    #[test]
    fn stream_without_a_headset_should_wait() {
        let mut snap = healthy();
        snap.link.state = LinkState::Searching;

        assert_eq!(note_of(&snap, Kind::Mic, 0), EN.waiting);
    }

    #[test]
    fn microphone_carrying_sound_should_say_so() {
        let mut snap = healthy();
        snap.link.receiving = 0b0100;

        assert_eq!(note_of(&snap, Kind::Mic, 2), EN.receiving);
    }

    #[test]
    fn switched_off_stream_should_not_show_as_live() {
        let mut snap = healthy();
        snap.link.sending = 0b0001;
        snap.outputs_on[0] = false;

        assert!(!tile_view(&snap, &EN, Kind::Output, 0, Touch::None).live);
    }

    #[test]
    fn microphone_should_carry_the_headsets_name() {
        let mut snap = healthy();
        snap.link.names = vec!["Discord".into()];

        assert_eq!(tile_view(&snap, &EN, Kind::Mic, 0, Touch::None).name, "Discord");
    }

    #[test]
    fn unnamed_microphone_should_carry_the_device_name() {
        let snap = healthy();

        assert_eq!(tile_view(&snap, &EN, Kind::Mic, 1, Touch::None).name, "Steam Streaming Mic B2");
    }

    #[test]
    fn connected_status_should_name_the_peer() {
        let mut snap = healthy();
        snap.link.peer = "frame".into();

        assert_eq!(status_view(&snap, &EN).title, "Connected to frame");
    }

    #[test]
    fn busy_refusal_should_be_explained() {
        let mut snap = healthy();
        snap.link.state = LinkState::Refused;
        snap.link.refused = "busy".into();

        assert_eq!(status_view(&snap, &EN).detail, EN.busy_detail);
    }

    #[test]
    fn unknown_refusal_should_show_the_reason_as_it_came() {
        let mut snap = healthy();
        snap.link.state = LinkState::Refused;
        snap.link.refused = "maintenance".into();

        assert_eq!(status_view(&snap, &EN).detail, "maintenance");
    }

    #[test]
    fn packed_svg_numbers_should_be_split() {
        let mut numbers = SvgNumbers { rest: "-.58.59-3,4 5e" };
        let read: Vec<f32> = std::iter::from_fn(|| numbers.next()).collect();

        assert_eq!(read, vec![-0.58, 0.59, -3.0, 4.0, 5.0]);
    }

    #[test]
    fn glyph_outline_should_parse() {
        assert!(svg_path(PC_SCREEN_FILL).is_some() && svg_path(PC_SCREEN_STROKE).is_some());
    }

    #[test]
    fn svg_path_with_an_unknown_command_should_be_rejected() {
        assert!(svg_path("M0 0A1 1 0 0 0 2 2").is_none());
    }

    #[test]
    fn relative_svg_line_should_end_where_it_says() {
        let bounds = svg_path("M10 10h5v-4").map(|path| path.bounds());

        assert_eq!(bounds.map(|bounds| (bounds.left(), bounds.top(), bounds.right(), bounds.bottom())), Some((10.0, 6.0, 15.0, 10.0)));
    }

    #[test]
    fn thumbnail_should_be_a_full_square() {
        assert_eq!(thumbnail().len(), (THUMBNAIL * THUMBNAIL * 4) as usize);
    }

    #[test]
    fn thumbnail_corner_should_be_transparent() {
        assert_eq!(thumbnail().get(3), Some(&0));
    }
}
