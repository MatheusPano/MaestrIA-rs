use std::rc::Rc;
use std::sync::Arc;

use gpui::{App, AppContext, Entity, Global, Window};
use mx_claude::usage::{UsageReading, read_usage, short_until};
use mx_plugins::PluginInfo;
use mx_plugins::catalog::PluginUpdate;
use mx_theme::Typography;

use crate::app::host::StoreHost;
use crate::keymap;
use crate::app::ui::{self, Layer};
use crate::app::dialogs;
use crate::chrome::settings::{
    Account, LauncherRow, PluginAuto, PluginMenuAction, PluginRow, PluginState, RowUpdate, Section, SettingsAction, SettingsData,
    SettingsDialog, UsageWindow, fixed_rows,
};

struct Usage(Account);

impl Global for Usage {}

struct Fonts(Rc<Vec<String>>);

impl Global for Fonts {}

fn fonts(cx: &mut App) -> Rc<Vec<String>> {
    if let Some(f) = cx.try_global::<Fonts>() {
        return f.0.clone();
    }
    let names = Rc::new(cx.text_system().all_font_names());
    cx.set_global(Fonts(names.clone()));
    names
}

pub fn theme_typography(t: &mx_core::persist::Typography, installed: &dyn Fn(&str) -> bool) -> Typography {
    Typography::new(Some(&t.mono), Some(t.size), Some(t.line), installed)
}

pub fn store_typography(t: &Typography) -> mx_core::persist::Typography {
    mx_core::persist::Typography::new(Some(t.mono()), Some(t.size()), Some(t.line()))
}

fn plugin_state(s: mx_plugins::PluginState) -> PluginState {
    use mx_plugins::PluginState as P;
    match s {
        P::Invalid => PluginState::Broken,
        P::Disabled => PluginState::Disabled,
        P::Idle => PluginState::Ready,
        P::Starting => PluginState::Starting,
        P::Running => PluginState::Running,
        P::Crashed => PluginState::Stopped,
    }
}

fn row_update(u: &PluginUpdate) -> RowUpdate {
    RowUpdate { from: u.from.clone(), to: u.to.clone(), asks_more: u.asks_more, newer: u.newer() }
}

fn plugin_row(p: &PluginInfo, conflicts: usize, store: &mx_store::Store) -> PluginRow {
    let m = p.manifest.as_deref();
    let registry = store.plugins();
    let updates = store.plugin_updates();
    let origin = registry.and_then(|r| r.origin_of(&p.id));
    let details = [p.problem.clone(), p.crash.clone()]
        .into_iter()
        .flatten()
        .chain(m.map(|m| m.warnings.clone()).unwrap_or_default())
        .chain(origin.as_ref().map(|o| o.describe()))
        .collect();
    PluginRow {
        id: p.id.clone(),
        name: p.name().to_owned(),
        version: m.map(|m| m.version.clone()).unwrap_or_default(),
        state: plugin_state(p.state),
        dev: p.linked,
        description: m.map(|m| m.description.clone()).unwrap_or_default(),
        details,
        commands: m.map(|m| m.commands.iter().map(|c| c.title.clone()).collect()).unwrap_or_default(),
        themes: p.palettes.clone(),
        enabled: p.enabled,
        conflicts,
        kinds: m.map(|m| m.kinds().into_iter().collect()).unwrap_or_default(),
        busy: updates.is_some_and(|u| u.is_busy(&p.id)),
        update: updates.and_then(|u| u.available().get(&p.id).map(row_update)),
        offer: updates.and_then(|u| u.offers().get(&p.id).map(row_update)),
        has_origin: origin.is_some(),
        manual: registry.is_some_and(|r| r.manual_update().contains(&p.id)),
    }
}

fn plugin_auto(store: &mx_store::Store) -> PluginAuto {
    let Some(u) = store.plugin_updates() else {
        return PluginAuto { on: store.plugins().is_none_or(|r| r.auto_update()), ..PluginAuto::default() };
    };
    PluginAuto {
        on: u.plugins().auto_update(),
        checking: u.checking(),
        checked_at: u.checked_at().map(|at| at.format("%H:%M").to_string()),
        error: u.error(),
    }
}

pub fn data(host: &Entity<StoreHost>, cx: &App) -> SettingsData {
    let store = &host.read(cx).store;
    let names = cx.try_global::<Fonts>().map(|f| f.0.clone()).unwrap_or_default();
    let installed = |family: &str| names.iter().any(|n| n == family);
    let prefs = store.prefs();
    let map = keymap::keymap_of(store);
    let open_with = |id: &str| store.tabs().iter().filter(|t| !t.exited && t.launcher_id.as_deref() == Some(id)).count();
    SettingsData {
        themes: mx_theme::built_ins().to_vec(),
        plugin_themes: store.plugins().map(|p| p.themes().extra().to_vec()).unwrap_or_default(),
        theme_id: prefs.theme.clone(),
        typography: theme_typography(&prefs.typography, &installed),
        faces: mx_theme::installed_faces(&installed),
        zoom_keys: keymap::zoom_keys(&map),
        status_bar_top: prefs.status_bar_top,
        shortcuts: keymap::shortcut_rows(&map),
        fixed: fixed_rows(true),
        launchers: store
            .launchers()
            .iter()
            .map(|l| LauncherRow { id: l.id.clone(), name: l.name.clone(), command: l.command.clone(), icon: l.icon, open: open_with(&l.id) })
            .collect(),
        plugins: store.plugins().map(|p| p.all().iter().map(|i| plugin_row(i, crate::plugin_ui::conflicts(store, &i.id), store)).collect()).unwrap_or_default(),
        plugin_auto: plugin_auto(store),
        hibernate_minutes: prefs.hibernate_minutes.max(0) as u32,
        account: cx.try_global::<Usage>().map_or(Account::Loading, |u| u.0.clone()),
    }
}

pub fn account_of(r: UsageReading, now: chrono::DateTime<chrono::Local>) -> Account {
    if let Some(problem) = r.problem {
        return Account::Error(problem);
    }
    Account::Ready {
        org: r.account.organization.unwrap_or_default(),
        plan: r.account.plan.unwrap_or_default(),
        email: r.account.email.unwrap_or_default(),
        windows: r
            .windows
            .into_iter()
            .map(|w| UsageWindow {
                label: w.label,
                percent: w.percent as f64,
                resets: w.resets_at.map(|at| short_until(&at, &now)).unwrap_or_default(),
            })
            .collect(),
        credits: r.credits,
        read_at: now.format("%H:%M").to_string(),
    }
}

fn refresh(cx: &mut App) {
    cx.set_global(Usage(Account::Loading));
    let host = ui::host(cx);
    let rt = host.read(cx).rt();
    let home = host.read(cx).store.options().home.clone().unwrap_or_default();
    let task = rt.spawn(async move {
        let claude = std::path::Path::new(&home).join(".claude");
        let json = std::path::Path::new(&home).join(".claude.json");
        read_usage(&claude, &json).await
    });
    cx.spawn(async move |cx| {
        let account = match task.await {
            Ok(r) => account_of(r, chrono::Local::now()),
            Err(err) => Account::Error(err.to_string()),
        };
        cx.update(|cx| {
            cx.set_global(Usage(account));
            push(cx);
        })
        .ok();
    })
    .detach();
}

fn push(cx: &mut App) {
    let host = ui::host(cx);
    let data = data(&host, cx);
    if let Some(Layer::Settings(d)) = ui::get(cx).read(cx).top() {
        d.update(cx, |d, cx| d.set_data(data, cx));
    }
}

pub fn open(section: Section, _window: &mut Window, cx: &mut App) {
    if let Some(Layer::Settings(d)) = ui::get(cx).read(cx).top() {
        d.update(cx, |d, cx| d.show(section, cx));
        return;
    }
    fonts(cx);
    if cx.try_global::<Usage>().is_none() || section == Section::Account {
        refresh(cx);
    }
    let host = ui::host(cx);
    let data = data(&host, cx);
    let view = cx.new(|cx| {
        SettingsDialog::new(data, section, Rc::new(keymap::check_chord), act, cx)
    });
    ui::show(|_| Layer::Settings(view), cx);
}

fn plugins(cx: &App) -> Option<mx_plugins::Plugins> {
    ui::store(cx).plugins().cloned()
}

fn on_rt(cx: &App, f: impl std::future::Future<Output = ()> + Send + 'static) {
    ui::host(cx).read(cx).rt().spawn(f);
}

fn run_shell(cx: &App, command: String) {
    on_rt(cx, async move {
        let runner = mx_sys::LoginShell;
        mx_core::ports::CommandRunner::run(&runner, command, None).await;
    });
}

fn act(action: SettingsAction, window: &mut Window, cx: &mut App) {
    use SettingsAction as S;
    match action {
        S::SetTheme(id) => ui::act(cx, |s| s.set_theme(&id)),
        S::SetTypography(t) => ui::act(cx, |s| s.set_typography(store_typography(&t))),
        S::SetStatusBarTop(top) => ui::act(cx, |s| s.set_status_bar_top(top)),
        S::ResetShortcuts(_) | S::Bind { .. } | S::Unbind { .. } => drop(ui::act(cx, |s| keymap::apply(s, &action))),
        S::NewLauncher => dialogs::launcher_form(None, None, window, cx),
        S::EditLauncher(id) => dialogs::launcher_form(Some(id), None, window, cx),
        S::RemoveLauncher(id) => ui::act(cx, |s| s.remove_launcher(&id)),
        S::SetPluginEnabled(id, on) => ui::act(cx, |s| s.set_plugin_enabled(&id, on)),
        S::SetHibernateMinutes(m) => ui::act(cx, |s| s.set_hibernate_minutes(m as i64)),
        S::ReloadUsage => {
            refresh(cx);
            push(cx);
        }
        S::Close => ui::close_settings(cx),
        S::OpenPluginsFolder => {
            if let Some(p) = plugins(cx) {
                std::fs::create_dir_all(p.root()).ok();
                run_shell(cx, format!("xdg-open {}", mx_sys::shell::q(&p.root().to_string_lossy())));
            }
        }
        S::RescanPlugins => {
            if let Some(p) = plugins(cx) {
                on_rt(cx, async move {
                    p.scan();
                    p.startup();
                });
            }
        }
        S::LoadDevPlugin => dialogs::pick(true, "Escolher", |picked, cx| {
            let (dialogs::Picked::Path(path), Some(p)) = (picked, plugins(cx)) else { return };
            let host = ui::host(cx);
            let task = host.read(cx).rt().spawn(async move { p.link(&path).await });
            cx.spawn(async move |cx| {
                let text = match task.await {
                    Ok(Ok(info)) => format!("{} carregado da pasta de desenvolvimento", info.name()),
                    Ok(Err(err)) => err.to_string(),
                    Err(err) => err.to_string(),
                };
                cx.update(|cx| ui::act(cx, |s| s.show_banner(&text))).ok();
            })
            .detach();
        }, cx),
        S::Plugin(id, PluginMenuAction::Restart) => {
            if let Some(p) = plugins(cx) {
                on_rt(cx, async move { p.restart(&id).await });
            }
        }
        S::Plugin(id, PluginMenuAction::Reveal) => {
            if let Some(dir) = plugins(cx).and_then(|p| p.get(&id)).map(|i| i.dir.to_string_lossy().into_owned()) {
                let runner: Arc<dyn mx_core::ports::CommandRunner> = Arc::new(mx_sys::LoginShell);
                on_rt(cx, async move {
                    mx_sys::reveal::reveal(runner.as_ref(), &dir).await;
                });
            }
        }
        S::Plugin(id, PluginMenuAction::Remove) => crate::plugin_ui::confirm_uninstall(&id, cx),
        S::Plugin(id, PluginMenuAction::Configure) => crate::plugin_ui::open_settings(&id, window, cx),
        S::Plugin(id, PluginMenuAction::Log) => crate::plugin_ui::open_log(&id, window, cx),
        S::InstallPlugin => crate::plugin_ui::open_install(window, cx),
        S::OpenCatalog => crate::plugin_ui::open_catalog(window, cx),
        S::SetPluginAutoUpdate(on) => ui::act(cx, |s| s.set_plugin_auto_update(on)),
        S::CheckPluginUpdates => ui::act(cx, |s| s.check_plugin_updates()),
        S::Plugin(id, PluginMenuAction::Update) => {
            if let Some(u) = ui::store(cx).plugin_updates().and_then(|u| u.available().remove(&id)) {
                crate::plugin_ui::update_plugin(u, cx);
            }
        }
        S::Plugin(id, PluginMenuAction::Adopt) => {
            if let Some(u) = ui::store(cx).plugin_updates().and_then(|u| u.offers().remove(&id)) {
                crate::plugin_ui::update_plugin(u, cx);
            }
        }
        S::Plugin(id, PluginMenuAction::ToggleAutoUpdate) => {
            let manual = ui::store(cx).plugins().is_some_and(|r| r.manual_update().contains(&id));
            ui::act(cx, |s| s.set_plugin_manual_update(&id, !manual));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mx_claude::usage::{UsageAccount, UsageWindow as Window};

    #[test]
    fn a_usage_problem_becomes_the_error_card() {
        let r = UsageReading { windows: vec![], account: UsageAccount::default(), credits: None, problem: Some("offline".into()) };
        assert_eq!(account_of(r, chrono::Local::now()), Account::Error("offline".into()));
    }

    #[test]
    fn usage_windows_keep_label_and_percent() {
        let r = UsageReading {
            windows: vec![Window { label: "Semana".into(), percent: 81, resets_at: None }],
            account: UsageAccount { email: Some("a@b".into()), organization: Some("Org".into()), plan: Some("Max".into()) },
            credits: None,
            problem: None,
        };
        let Account::Ready { org, plan, windows, .. } = account_of(r, chrono::Local::now()) else { panic!() };
        assert_eq!((org.as_str(), plan.as_str(), windows[0].percent), ("Org", "Max", 81.));
    }
}
