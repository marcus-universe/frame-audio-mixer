//! The name dialog: one text field that names a channel, a virtual microphone,
//! a project, or the VST folder. The text comes from the headset keyboard.

use super::{Action, NameEdit, Overlay, Panel};
use crate::config;

/// Longest VST folder path the dialog accepts.
const FOLDER_MAX_CHARS: usize = 200;

impl Panel {
    pub(super) fn begin_name(&mut self, mode: NameEdit, draft: String) -> Action {
        self.name_draft = draft;
        self.name_rejected = false;
        self.name_keyboard = false;
        self.overlay = Overlay::Name(mode);
        Action::None
    }

    /// Leaves the name dialog for `next`.
    fn end_name(&mut self, next: Overlay) {
        self.overlay = next;
        self.name_draft.clear();
        self.name_keyboard = false;
    }

    pub(super) fn commit_name(&mut self) -> Action {
        let Some(mode) = self.name_mode() else { return Action::None };
        let Ok(name) = config::normalize_name(&self.name_draft) else {
            self.name_rejected = true;
            return Action::None;
        };
        match mode {
            NameEdit::VstFolder => {
                let folder = self.name_draft.trim().to_string();
                if folder.is_empty() || folder.chars().count() > FOLDER_MAX_CHARS {
                    self.name_rejected = true;
                    return Action::None;
                }
                self.config.vst_folder = folder;
                self.end_name(Overlay::Vst);
                Action::ScanVst
            }
            NameEdit::Project => {
                if let Err(err) = config::save_project(&name, &self.config) {
                    eprintln!("frame-audio-mixer: project: {err}");
                }
                self.projects = config::list_projects();
                self.end_name(Overlay::Projects);
                Action::None
            }
            NameEdit::Bus(index) => {
                if let Some(bus) = self.config.b.get_mut(index) {
                    bus.name = name;
                }
                self.end_name(Overlay::None);
                Action::Save
            }
            NameEdit::Add => {
                if config::name_taken(&self.config.channels, &name, None) {
                    self.name_rejected = true;
                    return Action::None;
                }
                self.name_keyboard = false;
                self.add_channel(name, None)
            }
            NameEdit::Rename(id) => {
                if config::name_taken(&self.config.channels, &name, Some(id.as_str())) {
                    self.name_rejected = true;
                    return Action::None;
                }
                if let Some(channel) = self.config.channels.iter_mut().find(|c| c.id == id) {
                    channel.name = name;
                }
                self.end_name(Overlay::None);
                Action::Save
            }
        }
    }

    fn name_mode(&self) -> Option<NameEdit> {
        match &self.overlay {
            Overlay::Name(mode) => Some(mode.clone()),
            _ => None,
        }
    }

    /// The name dialog is saving a project, not naming a channel or bus.
    pub(crate) fn naming_project(&self) -> bool {
        matches!(self.overlay, Overlay::Name(NameEdit::Project))
    }

    /// The name dialog is editing the VST folder path.
    pub(crate) fn naming_folder(&self) -> bool {
        matches!(self.overlay, Overlay::Name(NameEdit::VstFolder))
    }

    fn text_limit(&self) -> usize {
        if self.naming_folder() { FOLDER_MAX_CHARS } else { config::NAME_MAX_CHARS }
    }

    /// The name field was pressed: ask the app for the dashboard keyboard.
    pub(super) fn show_keyboard(&mut self) -> Action {
        if !matches!(self.overlay, Overlay::Name(_)) {
            return Action::None;
        }
        self.name_keyboard = true;
        let t = crate::i18n::text(self.lang);
        let description = if self.naming_project() {
            t.project_name
        } else if self.naming_folder() {
            t.vst_folder
        } else {
            t.channel_name
        };
        Action::ShowKeyboard { description: description.to_string(), existing: self.name_draft.clone() }
    }

    /// True while the name dialog is waiting on the dashboard keyboard.
    pub fn keyboard_live(&self) -> bool {
        self.name_keyboard && matches!(self.overlay, Overlay::Name(_))
    }

    /// Copy headset-keyboard text into the name field.
    /// `full` is GetKeyboardText. `key` is the new key from a char event, which is not the whole line.
    pub fn apply_keyboard_text(&mut self, full: &str, key: &str, finished: bool) -> bool {
        if !matches!(self.overlay, Overlay::Name(_)) {
            self.name_keyboard = false;
            return false;
        }
        let next = if finished {
            self.name_keyboard = false;
            let full_n = full.chars().count();
            let draft_n = self.name_draft.chars().count();
            if !full.is_empty() && full_n >= draft_n {
                truncate_to(full, self.text_limit())
            } else {
                self.name_draft.clone()
            }
        } else {
            merge_keyboard_limit(&self.name_draft, full, key, self.text_limit())
        };
        if next == self.name_draft {
            return false;
        }
        self.name_draft = next;
        self.name_rejected = false;
        true
    }
}

fn truncate_to(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

fn merge_keyboard(draft: &str, full: &str, key: &str) -> String {
    merge_keyboard_limit(draft, full, key, config::NAME_MAX_CHARS)
}

/// The headset keyboard reports either the whole line (`full`) or only the new key (`key`).
fn merge_keyboard_limit(draft: &str, full: &str, key: &str, limit: usize) -> String {
    let truncate_name = |text: &str| truncate_to(text, limit);
    if key == "\u{8}" || key == "\u{7f}" {
        let mut chars: Vec<char> = draft.chars().collect();
        chars.pop();
        return chars.into_iter().collect();
    }
    let full_n = full.chars().count();
    let draft_n = draft.chars().count();
    if !full.is_empty() && full.starts_with(draft) && full_n > draft_n {
        return truncate_name(full);
    }
    // A one-character read is the newest key, not the line, so it must not wipe the name.
    if key.is_empty() && full_n > 1 && draft.starts_with(full) && draft_n > full_n && draft_n - full_n <= 2 {
        return truncate_name(full);
    }
    // Minimal mode replaces the buffer with the newest letter. A read of only the
    // first letter is a truncated line, so it must not be appended again.
    if key.is_empty() && full_n == 1 && !draft.is_empty() && !draft.starts_with(full) && !draft.ends_with(full) {
        return truncate_name(&format!("{draft}{full}"));
    }
    if !key.is_empty() {
        if !full.is_empty() && full.starts_with(draft) && full_n >= draft_n {
            let key_n = key.chars().count();
            // Already in the line, or the 8-byte event field repeating the start of it.
            if full.ends_with(key) || (key_n < full_n && full.starts_with(key)) {
                return truncate_name(full);
            }
        }
        if key.starts_with(draft) && key.chars().count() >= draft_n {
            return truncate_name(key);
        }
        return truncate_name(&format!("{draft}{key}"));
    }
    draft.to_string()
}

pub(super) fn self_test() -> Result<(), String> {
    let cases = [
        ("Mu", "Music", "s", "Music", "keyboard full line"),
        ("Musi", "c", "c", "Music", "keyboard one key"),
        ("Music", "M", "c", "Musicc", "keyboard keeps earlier letters"),
        ("Music", "Music", "c", "Music", "keyboard repeats the last key"),
        ("Music", "", "\u{8}", "Musi", "keyboard backspace"),
        ("M", "Music", "", "Music", "keyboard line grows"),
        ("A", "B", "", "AB", "keyboard newest letter"),
        ("Music", "M", "", "Music", "keyboard truncated line"),
        ("Music", "Music", "M", "Music", "keyboard event repeats the start"),
    ];
    for (draft, full, key, expected, what) in cases {
        if merge_keyboard(draft, full, key) != expected {
            return Err(what.into());
        }
    }
    Ok(())
}
