//! The menu-bar side of the app: the tray icon with the attention list, notifications when a
//! session finishes, needs the person or goes quiet, and the wake lock held while anything works.
//! It reacts to the attention list the pump sends about once a second.

use virgil_core::attention::anything_working;
use virgil_core::{AttentionItem, AttentionKind};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

pub const ATTENTION_CHANNEL: &str = "virgil:attention";
pub const FOCUS_CHANNEL: &str = "virgil:focus-session";
const MAIN_WINDOW: &str = "main";
const TRAY_ID: &str = "virgil-tray";
const MENU_LINE_MAX: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Prefs {
    /// Hold the computer awake while any session is working.
    pub keep_awake: bool,
    /// Notify when a session finishes, needs the person, or goes quiet.
    pub notifications: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs { keep_awake: true, notifications: true }
    }
}

/// What the UI gets on the attention channel and from `get_attention`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttentionState {
    pub items: Vec<AttentionItem>,
    pub prefs: Prefs,
    /// Whether the wake lock is held right now.
    pub awake: bool,
    /// Sessions that finished or asked something and have not been looked at yet.
    pub unseen: Vec<String>,
}

#[derive(Default)]
struct Inner {
    prefs: Prefs,
    prefs_path: Option<PathBuf>,
    items: Vec<AttentionItem>,
    /// Kind of each session at the last update. Empty until the first list arrives.
    kinds: HashMap<String, AttentionKind>,
    started: bool,
    unseen: HashSet<String>,
    wake: Option<keepawake::KeepAwake>,
    wake_error: Option<String>,
    tray: Option<TrayIcon>,
    menu_signature: String,
}

#[derive(Default)]
pub struct Tower(Mutex<Inner>);

/// A notification to show, decided by [`transitions`].
#[derive(Debug, PartialEq)]
pub struct Notice {
    pub title: String,
    pub body: String,
}

/// Which changes between two attention lists deserve a notification. A session that was not in
/// the previous list never does, so nothing fires at start-up or when a session first appears.
pub fn transitions(prev: &HashMap<String, AttentionKind>, items: &[AttentionItem]) -> Vec<(String, Notice)> {
    use AttentionKind::*;
    let mut out = Vec::new();
    for i in items {
        let Some(&was) = prev.get(&i.session_id) else { continue };
        if was == i.kind {
            continue;
        }
        let title = match (was, i.kind) {
            (_, NeedsYou) => format!("{} needs you", i.name),
            (Working | Quiet | NeedsYou, Finished) => format!("{} finished", i.name),
            (Working, Quiet) => format!("{} has gone quiet", i.name),
            _ => continue,
        };
        out.push((i.session_id.clone(), Notice { title, body: i.line.clone() }));
    }
    out
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn age(since: i64, now: i64) -> String {
    let mins = ((now - since) / 60_000).max(0);
    match mins {
        0 => "just now".into(),
        1..=59 => format!("{mins} min"),
        60..=1439 => format!("{} h", mins / 60),
        _ => format!("{} d", mins / 1440),
    }
}

fn cut(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('…');
    out
}

fn heading(kind: AttentionKind) -> &'static str {
    match kind {
        AttentionKind::NeedsYou => "Needs you",
        AttentionKind::Finished => "Finished",
        AttentionKind::Quiet => "Gone quiet",
        AttentionKind::Working => "Working",
        AttentionKind::Idle => "Idle",
    }
}

impl Tower {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Loads the saved preferences and creates the tray icon. Call once, from `setup`.
    pub fn init(&self, app: &AppHandle) {
        let mut g = self.lock();
        g.prefs_path = app.path().app_config_dir().ok().map(|d| d.join("prefs.json"));
        if let Some(saved) = g.prefs_path.as_ref().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()) {
            g.prefs = saved;
        }
        let mut builder = TrayIconBuilder::with_id(TRAY_ID).tooltip("Virgil").show_menu_on_left_click(true).on_menu_event(
            |app, event| {
                let id = event.id().as_ref().to_string();
                let tower = app.state::<Tower>();
                match id.as_str() {
                    "open" => show_main(app),
                    "quit" => app.exit(0),
                    "keep-awake" => tower.toggle(app, |p| p.keep_awake = !p.keep_awake),
                    "notifications" => tower.toggle(app, |p| p.notifications = !p.notifications),
                    other => {
                        if let Some(session_id) = other.strip_prefix("session:") {
                            tower.mark_seen(app, Some(session_id));
                            show_main(app);
                            let _ = app.emit_to(MAIN_WINDOW, FOCUS_CHANNEL, session_id.to_string());
                        }
                    }
                }
            },
        );
        if let Some(icon) = app.default_window_icon() {
            builder = builder.icon(icon.clone());
        }
        match builder.build(app) {
            Ok(tray) => g.tray = Some(tray),
            Err(e) => eprintln!("virgil: tray icon: {e}"),
        }
        drop(g);
        self.refresh(app, true);
    }

    pub fn state(&self) -> AttentionState {
        let g = self.lock();
        let mut unseen: Vec<String> = g.unseen.iter().cloned().collect();
        unseen.sort();
        AttentionState { items: g.items.clone(), prefs: g.prefs, awake: g.wake.is_some(), unseen }
    }

    pub fn set_prefs(&self, app: &AppHandle, prefs: Prefs) {
        self.toggle(app, |p| *p = prefs);
    }

    fn toggle(&self, app: &AppHandle, change: impl FnOnce(&mut Prefs)) {
        {
            let mut g = self.lock();
            change(&mut g.prefs);
            if let Some(path) = &g.prefs_path {
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if let Ok(json) = serde_json::to_vec_pretty(&g.prefs) {
                    let _ = std::fs::write(path, json);
                }
            }
        }
        self.refresh(app, true);
    }

    /// Marks one session, or all of them, as looked at.
    pub fn mark_seen(&self, app: &AppHandle, session_id: Option<&str>) {
        {
            let mut g = self.lock();
            match session_id {
                Some(id) => {
                    g.unseen.remove(id);
                }
                None => g.unseen.clear(),
            }
        }
        self.refresh(app, true);
    }

    /// A new attention list from the pump.
    pub fn update(&self, app: &AppHandle, items: Vec<AttentionItem>) {
        let notices = {
            let mut g = self.lock();
            let notices = if g.started { transitions(&g.kinds, &items) } else { Vec::new() };
            g.started = true;
            g.kinds = items.iter().map(|i| (i.session_id.clone(), i.kind)).collect();
            // Unseen lasts only while the session stays finished or asking.
            let still: HashSet<&str> = items
                .iter()
                .filter(|i| matches!(i.kind, AttentionKind::Finished | AttentionKind::NeedsYou))
                .map(|i| i.session_id.as_str())
                .collect();
            g.unseen.retain(|id| still.contains(id.as_str()));
            for (id, _) in &notices {
                if still.contains(id.as_str()) {
                    g.unseen.insert(id.clone());
                }
            }
            g.items = items;
            if g.prefs.notifications {
                notices
            } else {
                Vec::new()
            }
        };
        for (_, n) in notices {
            eprintln!("virgil: notify: {} / {}", n.title, n.body);
            if let Err(e) = app.notification().builder().title(&n.title).body(&n.body).show() {
                eprintln!("virgil: notification: {e}");
            }
        }
        self.refresh(app, false);
    }

    /// Applies the wake lock, redraws the tray, and tells the window.
    fn refresh(&self, app: &AppHandle, force: bool) {
        let mut g = self.lock();
        let want_awake = g.prefs.keep_awake && anything_working(&g.items);
        if want_awake && g.wake.is_none() {
            match keepawake::Builder::default()
                .idle(true)
                .reason("Claude Code sessions are working")
                .app_name("Virgil")
                .app_reverse_domain("io.github.dader.virgil")
                .create()
            {
                Ok(k) => {
                    eprintln!("virgil: wake lock on");
                    g.wake = Some(k);
                    g.wake_error = None;
                }
                Err(e) => {
                    let text = e.to_string();
                    if g.wake_error.as_deref() != Some(text.as_str()) {
                        eprintln!("virgil: wake lock: {text}");
                    }
                    g.wake_error = Some(text);
                }
            }
        } else if !want_awake && g.wake.is_some() {
            eprintln!("virgil: wake lock off");
            g.wake = None;
        }

        let now = now_ms();
        let awake = g.wake.is_some();
        let signature = format!(
            "{:?}|{awake}|{:?}|{}",
            g.prefs,
            g.unseen.len(),
            g.items.iter().map(|i| format!("{}:{:?}:{}:{}", i.session_id, i.kind, i.line, age(i.since, now))).collect::<Vec<_>>().join("|")
        );
        if !force && signature == g.menu_signature {
            return;
        }
        g.menu_signature = signature;
        let count = |k: AttentionKind| g.items.iter().filter(|i| i.kind == k).count();
        eprintln!(
            "virgil: attention: {} need you, {} finished, {} quiet, {} working, {} idle, {} unseen",
            count(AttentionKind::NeedsYou),
            count(AttentionKind::Finished),
            count(AttentionKind::Quiet),
            count(AttentionKind::Working),
            count(AttentionKind::Idle),
            g.unseen.len()
        );
        if let Some(tray) = &g.tray {
            if let Err(e) = build_menu(app, &g, now).and_then(|m| tray.set_menu(Some(m))) {
                eprintln!("virgil: tray menu: {e}");
            }
            let needs = g.items.iter().filter(|i| i.kind == AttentionKind::NeedsYou).count();
            let title = match (needs, g.unseen.len()) {
                (0, 0) => None,
                (n, u) => Some(format!("{}", n.max(u))),
            };
            let _ = tray.set_title(title);
            let working = g.items.iter().filter(|i| matches!(i.kind, AttentionKind::Working | AttentionKind::Quiet)).count();
            let _ = tray.set_tooltip(Some(format!("Virgil: {working} working, {needs} need you")));
        }
        let state = AttentionState {
            items: g.items.clone(),
            prefs: g.prefs,
            awake,
            unseen: g.unseen.iter().cloned().collect(),
        };
        drop(g);
        let _ = app.emit_to(MAIN_WINDOW, ATTENTION_CHANNEL, state);
    }
}

fn build_menu(app: &AppHandle, g: &Inner, now: i64) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let mut menu = MenuBuilder::new(app);
    let mut any = false;
    for kind in [AttentionKind::NeedsYou, AttentionKind::Finished, AttentionKind::Quiet, AttentionKind::Working] {
        let group: Vec<&AttentionItem> = g.items.iter().filter(|i| i.kind == kind).collect();
        if group.is_empty() {
            continue;
        }
        if any {
            menu = menu.separator();
        }
        any = true;
        menu = menu.item(&MenuItemBuilder::with_id(format!("head:{kind:?}"), heading(kind)).enabled(false).build(app)?);
        for i in group {
            let dot = if g.unseen.contains(&i.session_id) { "● " } else { "" };
            let label = format!("{dot}{}  ·  {}  ·  {}", i.name, cut(&i.line, MENU_LINE_MAX), age(i.since, now));
            menu = menu.item(&MenuItemBuilder::with_id(format!("session:{}", i.session_id), label).build(app)?);
        }
    }
    if !any {
        let idle = g.items.len();
        let text = if g.started { format!("Nothing running ({idle} idle)") } else { "Reading sessions…".to_string() };
        menu = menu.item(&MenuItemBuilder::with_id("head:none", text).enabled(false).build(app)?);
    }
    let awake_label = if g.wake.is_some() { "Keep awake while sessions work (on now)" } else { "Keep awake while sessions work" };
    menu = menu
        .separator()
        .item(&CheckMenuItemBuilder::with_id("keep-awake", awake_label).checked(g.prefs.keep_awake).build(app)?)
        .item(
            &CheckMenuItemBuilder::with_id("notifications", "Notify when a session finishes or needs me")
                .checked(g.prefs.notifications)
                .build(app)?,
        )
        .separator()
        .item(&MenuItemBuilder::with_id("open", "Open Virgil").build(app)?)
        .item(&MenuItemBuilder::with_id("quit", "Quit Virgil").build(app)?);
    menu.build()
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN_WINDOW) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, kind: AttentionKind) -> AttentionItem {
        AttentionItem {
            session_id: id.into(),
            name: format!("name-{id}"),
            project: None,
            title: None,
            kind,
            since: 0,
            line: "where it stands".into(),
            running_agents: 0,
            background_tasks: 0,
        }
    }

    #[test]
    fn notifies_on_finish_question_and_quiet_but_not_at_first_sight() {
        use AttentionKind::*;
        let prev: HashMap<String, AttentionKind> =
            [("a", Working), ("b", Working), ("c", Working), ("d", Finished), ("e", Idle), ("f", Finished)]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect();
        let items = vec![
            item("a", Finished),
            item("b", NeedsYou),
            item("c", Quiet),
            item("d", Idle),     // finished long ago: silent
            item("e", Finished), // idle to finished is not news
            item("f", Working),  // picked up again: silent
            item("new", Finished),
        ];
        let titles: Vec<String> = transitions(&prev, &items).into_iter().map(|(_, n)| n.title).collect();
        assert_eq!(titles, vec!["name-a finished", "name-b needs you", "name-c has gone quiet"]);
        assert!(transitions(&HashMap::new(), &items).is_empty());
    }

    #[test]
    fn ages_are_short() {
        assert_eq!(age(0, 30_000), "just now");
        assert_eq!(age(0, 12 * 60_000), "12 min");
        assert_eq!(age(0, 150 * 60_000), "2 h");
        assert_eq!(age(0, 3 * 1440 * 60_000), "3 d");
    }
}
