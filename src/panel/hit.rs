//! What the pointer can press, grab or move.
//!
//! Every clickable area is recorded as a [`Hit`] while the frame is drawn, and
//! [`Panel::handle`](super::Panel) matches on it. Nothing is encoded in strings,
//! so a frame allocates nothing for its hit areas and a click parses nothing.
//!
//! Channels are named by their position in `config.channels`. The panel drops
//! every recorded hit whenever that list changes, so a position is never stale.

use super::menu::Menu;
use super::{Drawer, Page};
use crate::config::EffectKind;
use crate::i18n::Lang;

/// A signal that can be routed: the microphone or a channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    Mic,
    /// Position in `config.channels`.
    Channel(usize),
}

/// An output: the PC streaming microphone, a listen output A1..A4, or a virtual microphone B1..B4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BusId {
    Pc,
    A(usize),
    B(usize),
}

impl BusId {
    /// The spelling the config and the worker use: `pc`, `a:0`, `b:2`.
    pub fn parse(value: &str) -> BusId {
        if let Some(rest) = value.strip_prefix("a:") {
            return BusId::A(rest.parse().unwrap_or(0));
        }
        if let Some(rest) = value.strip_prefix("b:") {
            return BusId::B(rest.parse().unwrap_or(0));
        }
        BusId::Pc
    }

    pub fn value(self) -> String {
        match self {
            BusId::Pc => "pc".into(),
            BusId::A(index) => format!("a:{index}"),
            BusId::B(index) => format!("b:{index}"),
        }
    }

    /// `A1`, `B3`. The PC output has no tag.
    pub fn tag(self) -> String {
        match self {
            BusId::Pc => String::new(),
            BusId::A(index) => format!("A{}", index + 1),
            BusId::B(index) => format!("B{}", index + 1),
        }
    }
}

/// One slider of the selected EQ band.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EqSlider {
    Freq,
    Gain,
    Q,
}

/// Anything that is dragged like a fader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FaderId {
    Mic,
    /// Position in `config.channels`.
    Channel(usize),
    Bus(BusId),
    /// Level of an effect bus, after its chain.
    FxLevel(usize),
    /// One parameter of an effect in a chain.
    FxParam { bus: usize, index: usize, param: usize },
    Eq(EqSlider),
}

/// What a held pointer is moving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Drag {
    Fader(FaderId),
    /// The scrollbar thumb of the channel strip.
    Scrollbar,
    /// Empty space of the channel strip.
    PanChannels,
    /// Empty space of the Outputs drawer.
    PanOutputs,
    /// The Signal Flow canvas.
    PanFlow,
    /// The grip between the mixer and the drawer.
    Split,
    /// The grip of an effect, to reorder its chain.
    FxGrip,
    /// A band handle in the EQ graph.
    EqBand(usize),
}

impl Drag {
    /// Scrolling changes nothing worth writing to disk.
    pub fn saves_on_release(self) -> bool {
        !matches!(self, Drag::Scrollbar | Drag::PanChannels | Drag::PanOutputs | Drag::PanFlow)
    }
}

/// One of the two Mix Check selectors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pick {
    /// The output a take records.
    Record,
    /// The listen output a take plays on.
    Play,
}

/// A clickable area. The few variants that carry text belong to dialogs and
/// lists, which are only drawn while they are open.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Hit {
    /// Swallows the click, so a dialog surface does not dismiss itself.
    Noop,
    /// Closes the open dialog.
    Dismiss,

    // Header and footer.
    Page(Page),
    Back,
    Wide(bool),
    Projects,
    LanguageMenu,
    Language(Lang),
    Settings,
    Autostart(bool),
    Quit,

    // Settings dialog.
    Meters(bool),
    AccentMenu,
    /// `blue` or `pink`.
    Accent(&'static str),
    VstSettings,
    /// The link to Frame Audio Link on the PC, on or off.
    PcLink(bool),

    // Mixer card.
    Snap(bool),
    /// Channels as rows instead of vertical strips.
    InputRows(bool),
    /// Outputs as columns instead of boxes.
    OutputColumns(bool),
    Split,
    Drawer(Drawer),
    MicPicker,
    /// A source node name, `builtin`, or empty for the system default.
    MicSource(String),
    AddChannel,
    RenameChannel(usize),
    /// First press arms, the second removes.
    RemoveChannel(usize),
    Mute(Source),
    /// Toggles one route of a source.
    Route { from: Source, to: BusId },
    /// Switches an effect bus (0 or 1) on or off for a source. The effect's result
    /// replaces the source's signal on every output the source is routed to.
    Insert { from: Source, fx: usize },

    // Add-channel dialog.
    RefreshApps,
    CustomName,
    /// A running app, by name.
    UseApp(String),
    Suggest(&'static str),
    /// A channel that plays PC output PC1..PC4, by its index.
    AddPcInput(usize),

    // Name dialog.
    NameField,
    NameOk,

    // Outputs drawer and Mix Check.
    BusMute(BusId),
    /// Rename a virtual microphone B1..B4.
    BusRename(usize),
    Record,
    StopCheck,
    PlayTake,
    PauseTake,
    /// Opens or closes a Mix Check selector.
    PickMenu(Pick),
    /// The open list itself, so a click between its rows keeps it open.
    PickMenuSurface(Pick),
    PickBus(Pick, BusId),

    // Effect drawers.
    FxPicker(usize),
    FxAdd { bus: usize, kind: EffectKind },
    /// A scanned plugin, by its position in the scan.
    FxAddVst { bus: usize, plugin: usize },
    FxRemove { bus: usize, index: usize },
    FxEdit { bus: usize, index: usize },
    FxPitchSnap { bus: usize, index: usize },
    FxGrip { bus: usize, index: usize },
    /// Opens the editor of the plugin with this instance id.
    VstEditor(String),
    VstFolder,
    VstScan,

    // Dropdown menus.
    OpenMenu(Menu),
    MenuClose,
    /// A row of `EqView::ALL`.
    PickEqView(usize),
    PickPreset { bus: usize, index: usize, row: usize },

    // EQ sheet.
    EqBand(usize),
    EqPoints(u8),
    EqReset,
    EqBypass,
    EqDone,
    EqType,
    EqBandOn,

    // Projects dialog.
    ProjectSave,
    ProjectLoad(usize),
    /// First press arms, the second deletes.
    ProjectRemove(usize),

    // Signal Flow page.
    FlowApps,
    FlowNode(String),
    FlowZoomIn,
    FlowZoomOut,
    FlowFit,

    // Apps & updates page.
    CheckUpdates(bool),
    CheckUpdatesNow,
    InstallUpdate,
    AppsList,
    /// An app key. Empty opens the installer for all of them.
    AppInstall(String),
    ConfirmNo,
    ConfirmYes(String),
}
