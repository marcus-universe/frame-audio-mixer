//! Seven languages. "auto" follows Steam's language, then the locale.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Ja,
    En,
    Sc,
    De,
    Fr,
    Es,
    It,
}

pub const LANGS: [Lang; 7] = [Lang::Ja, Lang::En, Lang::Sc, Lang::De, Lang::Fr, Lang::Es, Lang::It];

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::Ja => "ja",
            Lang::En => "en",
            Lang::Sc => "sc",
            Lang::De => "de",
            Lang::Fr => "fr",
            Lang::Es => "es",
            Lang::It => "it",
        }
    }

    pub fn endonym(self) -> &'static str {
        match self {
            Lang::Ja => "日本語",
            Lang::En => "English",
            Lang::Sc => "简体中文",
            Lang::De => "Deutsch",
            Lang::Fr => "Français",
            Lang::Es => "Español",
            Lang::It => "Italiano",
        }
    }
}

pub fn parse_code(code: &str) -> Option<Lang> {
    LANGS.into_iter().find(|lang| lang.code() == code)
}

pub fn resolve(configured: &str) -> Lang {
    if configured != "auto" {
        if let Some(lang) = parse_code(configured) {
            return lang;
        }
    }
    detect()
}

fn detect() -> Lang {
    if let Ok(steam) = std::env::var("STEAM_LANGUAGE") {
        match steam.to_lowercase().as_str() {
            "japanese" => return Lang::Ja,
            "schinese" | "tchinese" => return Lang::Sc,
            "german" => return Lang::De,
            "french" => return Lang::Fr,
            "spanish" | "latam" => return Lang::Es,
            "italian" => return Lang::It,
            "english" => return Lang::En,
            _ => {}
        }
    }
    let locale = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default()
        .to_lowercase();
    let prefix = locale.split(['_', '.', '@']).next().unwrap_or("");
    match prefix {
        "ja" => Lang::Ja,
        "zh" => Lang::Sc,
        "de" => Lang::De,
        "fr" => Lang::Fr,
        "es" => Lang::Es,
        "it" => Lang::It,
        _ => Lang::En,
    }
}

pub struct Text {
    pub title: &'static str,
    pub apps: &'static str,
    pub back: &'static str,
    pub on: &'static str,
    pub off: &'static str,
    pub status_on: &'static str,
    pub status_off: &'static str,
    pub mic: &'static str,
    pub to_mic: &'static str,
    pub phones: &'static str,
    pub add: &'static str,
    pub picker: &'static str,
    pub suggest: &'static str,
    pub running: &'static str,
    pub custom: &'static str,
    pub pick_channel: &'static str,
    pub pinned: &'static str,
    pub mix_check: &'static str,
    pub mix_check_body: &'static str,
    pub record: &'static str,
    pub stop: &'static str,
    pub play: &'static str,
    pub mix_check_need: &'static str,
    pub echo_hint: &'static str,
    pub footer_start: &'static str,
    pub footer_quit: &'static str,
    pub quit_confirm: &'static str,
    pub quit_yes: &'static str,
    pub cancel: &'static str,
    pub apps_check: &'static str,
    pub apps_update: &'static str,
    pub apps_current: &'static str,
    pub apps_available: &'static str,
    pub apps_list: &'static str,
    pub not_installed: &'static str,
    pub installed: &'static str,
    pub channel_name: &'static str,
    pub mic_default: &'static str,
    pub name_rejected: &'static str,
}

pub fn text(lang: Lang) -> &'static Text {
    match lang {
        Lang::En => &EN,
        Lang::Ja => &JA,
        Lang::Sc => &SC,
        Lang::De => &DE,
        Lang::Fr => &FR,
        Lang::Es => &ES,
        Lang::It => &IT,
    }
}

const EN: Text = Text {
    title: "Mix",
    apps: "Apps & updates",
    back: "Back",
    on: "On",
    off: "Off",
    status_on: "The PC hears the microphone and the channels.",
    status_off: "Mix is off. The PC hears the normal microphone.",
    mic: "Microphone",
    to_mic: "To mic",
    phones: "Headphones",
    add: "Add",
    picker: "Send an app to the selected channel",
    suggest: "Suggestions",
    running: "Playing now",
    custom: "Custom name",
    pick_channel: "Select a channel first.",
    pinned: "Pinned",
    mix_check: "Mix Check",
    mix_check_body: "Records Frame Mic + Mix for up to 10 seconds, then plays it on the current output.",
    record: "Record",
    stop: "Stop",
    play: "Play",
    mix_check_need: "Turn Mix on to record.",
    echo_hint: "The output is the Frame speakers and echo cancellation is off, so this recording can contain the speakers twice. Turn echo cancellation on in Frame Mic Tuner.",
    footer_start: "Start with SteamVR",
    footer_quit: "Quit",
    quit_confirm: "Quit Frame Audio Mixer?",
    quit_yes: "Quit",
    cancel: "Cancel",
    apps_check: "Check for updates",
    apps_update: "Update",
    apps_current: "Up to date",
    apps_available: "Update available",
    apps_list: "On this headset",
    not_installed: "Not installed",
    installed: "Installed",
    channel_name: "Channel name",
    mic_default: "Default microphone",
    name_rejected: "That name can't be used.",
};

const JA: Text = Text {
    title: "Mix",
    apps: "アプリと更新",
    back: "戻る",
    on: "オン",
    off: "オフ",
    status_on: "PC にはマイクとチャンネルが届きます。",
    status_off: "ミックスはオフです。PC には通常のマイクが届きます。",
    mic: "マイク",
    to_mic: "マイクへ",
    phones: "ヘッドホン",
    add: "追加",
    picker: "選んだチャンネルへアプリを送る",
    suggest: "候補",
    running: "再生中",
    custom: "名前を入力",
    pick_channel: "先にチャンネルを選んでください。",
    pinned: "固定中",
    mix_check: "ミックス確認",
    mix_check_body: "Frame Mic + Mix を最大 10 秒録音し、今の出力で再生します。",
    record: "録音",
    stop: "停止",
    play: "再生",
    mix_check_need: "録音するにはミックスをオンにしてください。",
    echo_hint: "出力が Frame のスピーカーで、エコー除去がオフです。録音にスピーカーの音が二重に入ることがあります。Frame Mic Tuner でエコー除去をオンにしてください。",
    footer_start: "SteamVR と一緒に起動",
    footer_quit: "終了",
    quit_confirm: "Frame Audio Mixer を終了しますか？",
    quit_yes: "終了",
    cancel: "キャンセル",
    apps_check: "更新を確認",
    apps_update: "更新",
    apps_current: "最新です",
    apps_available: "更新があります",
    apps_list: "このヘッドセット",
    not_installed: "未インストール",
    installed: "インストール済み",
    channel_name: "チャンネル名",
    mic_default: "既定のマイク",
    name_rejected: "その名前は使えません。",
};

const SC: Text = Text {
    title: "Mix",
    apps: "应用和更新",
    back: "返回",
    on: "开",
    off: "关",
    status_on: "电脑会听到麦克风和各个通道。",
    status_off: "混音已关闭。电脑听到的是普通麦克风。",
    mic: "麦克风",
    to_mic: "送到麦克风",
    phones: "耳机",
    add: "添加",
    picker: "把应用送到选中的通道",
    suggest: "建议",
    running: "正在播放",
    custom: "自定义名称",
    pick_channel: "请先选择一个通道。",
    pinned: "已固定",
    mix_check: "混音检查",
    mix_check_body: "录制 Frame Mic + Mix 最多 10 秒，然后在当前输出上播放。",
    record: "录制",
    stop: "停止",
    play: "播放",
    mix_check_need: "打开混音后才能录制。",
    echo_hint: "当前输出是 Frame 扬声器，且回声消除已关闭，录音里可能出现两份扬声器声音。请在 Frame Mic Tuner 里打开回声消除。",
    footer_start: "随 SteamVR 启动",
    footer_quit: "退出",
    quit_confirm: "要退出 Frame Audio Mixer 吗？",
    quit_yes: "退出",
    cancel: "取消",
    apps_check: "检查更新",
    apps_update: "更新",
    apps_current: "已是最新",
    apps_available: "有可用更新",
    apps_list: "这台头显上",
    not_installed: "未安装",
    installed: "已安装",
    channel_name: "通道名称",
    mic_default: "默认麦克风",
    name_rejected: "这个名称不能使用。",
};

const DE: Text = Text {
    title: "Mix",
    apps: "Apps und Updates",
    back: "Zurück",
    on: "An",
    off: "Aus",
    status_on: "Der PC hört das Mikrofon und die Kanäle.",
    status_off: "Mix ist aus. Der PC hört das normale Mikrofon.",
    mic: "Mikrofon",
    to_mic: "Zum Mikro",
    phones: "Kopfhörer",
    add: "Hinzufügen",
    picker: "App auf den gewählten Kanal legen",
    suggest: "Vorschläge",
    running: "Läuft gerade",
    custom: "Eigener Name",
    pick_channel: "Zuerst einen Kanal wählen.",
    pinned: "Festgelegt",
    mix_check: "Mix-Check",
    mix_check_body: "Nimmt Frame Mic + Mix bis zu 10 Sekunden auf und spielt es auf dem aktuellen Ausgang ab.",
    record: "Aufnahme",
    stop: "Stopp",
    play: "Wiedergabe",
    mix_check_need: "Mix einschalten, um aufzunehmen.",
    echo_hint: "Der Ausgang sind die Frame-Lautsprecher und die Echounterdrückung ist aus. Die Aufnahme kann die Lautsprecher doppelt enthalten. Echounterdrückung in Frame Mic Tuner einschalten.",
    footer_start: "Mit SteamVR starten",
    footer_quit: "Beenden",
    quit_confirm: "Frame Audio Mixer beenden?",
    quit_yes: "Beenden",
    cancel: "Abbrechen",
    apps_check: "Nach Updates suchen",
    apps_update: "Aktualisieren",
    apps_current: "Aktuell",
    apps_available: "Update verfügbar",
    apps_list: "Auf diesem Headset",
    not_installed: "Nicht installiert",
    installed: "Installiert",
    channel_name: "Kanalname",
    mic_default: "Standardmikrofon",
    name_rejected: "Dieser Name geht nicht.",
};

const FR: Text = Text {
    title: "Mix",
    apps: "Applis et mises à jour",
    back: "Retour",
    on: "Oui",
    off: "Non",
    status_on: "Le PC entend le micro et les canaux.",
    status_off: "Le mix est arrêté. Le PC entend le micro habituel.",
    mic: "Micro",
    to_mic: "Vers le micro",
    phones: "Casque",
    add: "Ajouter",
    picker: "Envoyer une appli vers le canal choisi",
    suggest: "Suggestions",
    running: "En lecture",
    custom: "Nom personnalisé",
    pick_channel: "Choisissez d'abord un canal.",
    pinned: "Épinglé",
    mix_check: "Contrôle du mix",
    mix_check_body: "Enregistre Frame Mic + Mix pendant 10 secondes au plus, puis le joue sur la sortie actuelle.",
    record: "Enregistrer",
    stop: "Stop",
    play: "Lecture",
    mix_check_need: "Activez le mix pour enregistrer.",
    echo_hint: "La sortie est les haut-parleurs du Frame et l'annulation d'écho est coupée : l'enregistrement peut contenir les haut-parleurs deux fois. Activez l'annulation d'écho dans Frame Mic Tuner.",
    footer_start: "Démarrer avec SteamVR",
    footer_quit: "Quitter",
    quit_confirm: "Quitter Frame Audio Mixer ?",
    quit_yes: "Quitter",
    cancel: "Annuler",
    apps_check: "Chercher des mises à jour",
    apps_update: "Mettre à jour",
    apps_current: "À jour",
    apps_available: "Mise à jour disponible",
    apps_list: "Sur ce casque",
    not_installed: "Pas installé",
    installed: "Installé",
    channel_name: "Nom du canal",
    mic_default: "Micro par défaut",
    name_rejected: "Ce nom ne convient pas.",
};

const ES: Text = Text {
    title: "Mix",
    apps: "Apps y actualizaciones",
    back: "Volver",
    on: "Sí",
    off: "No",
    status_on: "El PC oye el micrófono y los canales.",
    status_off: "La mezcla está apagada. El PC oye el micrófono normal.",
    mic: "Micrófono",
    to_mic: "Al micrófono",
    phones: "Auriculares",
    add: "Añadir",
    picker: "Enviar una app al canal elegido",
    suggest: "Sugerencias",
    running: "Sonando ahora",
    custom: "Nombre propio",
    pick_channel: "Elige primero un canal.",
    pinned: "Fijado",
    mix_check: "Prueba de mezcla",
    mix_check_body: "Graba Frame Mic + Mix hasta 10 segundos y lo reproduce en la salida actual.",
    record: "Grabar",
    stop: "Parar",
    play: "Reproducir",
    mix_check_need: "Activa la mezcla para grabar.",
    echo_hint: "La salida son los altavoces del Frame y la cancelación de eco está apagada, así que la grabación puede incluir los altavoces dos veces. Actívala en Frame Mic Tuner.",
    footer_start: "Iniciar con SteamVR",
    footer_quit: "Salir",
    quit_confirm: "¿Salir de Frame Audio Mixer?",
    quit_yes: "Salir",
    cancel: "Cancelar",
    apps_check: "Buscar actualizaciones",
    apps_update: "Actualizar",
    apps_current: "Al día",
    apps_available: "Hay una actualización",
    apps_list: "En este visor",
    not_installed: "No instalada",
    installed: "Instalada",
    channel_name: "Nombre del canal",
    mic_default: "Micrófono predeterminado",
    name_rejected: "Ese nombre no vale.",
};

const IT: Text = Text {
    title: "Mix",
    apps: "App e aggiornamenti",
    back: "Indietro",
    on: "Sì",
    off: "No",
    status_on: "Il PC sente il microfono e i canali.",
    status_off: "Il mix è spento. Il PC sente il microfono normale.",
    mic: "Microfono",
    to_mic: "Verso il mic",
    phones: "Cuffie",
    add: "Aggiungi",
    picker: "Manda un'app al canale scelto",
    suggest: "Suggeriti",
    running: "In riproduzione",
    custom: "Nome personalizzato",
    pick_channel: "Prima scegli un canale.",
    pinned: "Fissato",
    mix_check: "Prova mix",
    mix_check_body: "Registra Frame Mic + Mix per al massimo 10 secondi e lo riproduce sull'uscita attuale.",
    record: "Registra",
    stop: "Stop",
    play: "Riproduci",
    mix_check_need: "Attiva il mix per registrare.",
    echo_hint: "L'uscita sono gli altoparlanti del Frame e la cancellazione dell'eco è spenta: la registrazione può contenere gli altoparlanti due volte. Attivala in Frame Mic Tuner.",
    footer_start: "Avvia con SteamVR",
    footer_quit: "Esci",
    quit_confirm: "Uscire da Frame Audio Mixer?",
    quit_yes: "Esci",
    cancel: "Annulla",
    apps_check: "Cerca aggiornamenti",
    apps_update: "Aggiorna",
    apps_current: "Aggiornato",
    apps_available: "Aggiornamento disponibile",
    apps_list: "Su questo visore",
    not_installed: "Non installata",
    installed: "Installata",
    channel_name: "Nome del canale",
    mic_default: "Microfono predefinito",
    name_rejected: "Quel nome non va bene.",
};

pub fn self_test() -> Result<(), String> {
    for lang in LANGS {
        let code = lang.code();
        if parse_code(code) != Some(lang) {
            return Err(code.into());
        }
        let t = text(lang);
        if t.title.is_empty() || t.footer_start.is_empty() || t.apps.is_empty() {
            return Err(format!("empty {code}"));
        }
    }
    if parse_code("mix").is_some() {
        return Err("mix is not a language".into());
    }
    Ok(())
}
