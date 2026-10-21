use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mx_theme::{Palette, Themes};

use super::{Entry, Plugins, Registry};
use crate::manifest::PluginManifest;
use crate::process::SHUTDOWN_GRACE;

fn plugin_dirs(root: &Path) -> Vec<(PathBuf, bool)> {
    let Ok(read) = fs::read_dir(root) else { return Vec::new() };
    let mut out: Vec<(PathBuf, bool)> = read
        .filter_map(Result::ok)
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .filter_map(|e| {
            let path = e.path();
            let meta = fs::symlink_metadata(&path).ok()?;
            if meta.is_dir() {
                Some((path, false))
            } else if meta.file_type().is_symlink() && path.is_dir() {
                Some((path, true))
            } else {
                None
            }
        })
        .collect();
    out.sort_by(|a, b| a.0.as_os_str().cmp(b.0.as_os_str()));
    out
}

impl Plugins {
    pub fn scan(&self) {
        let mut stops = Vec::new();
        {
            let mut reg = self.reg();
            reg.scanned = true;
            let mut previous: HashMap<PathBuf, Entry> =
                reg.all.drain(..).map(|e| (e.dir.clone(), e)).collect();
            for (dir, linked) in plugin_dirs(&self.inner.root) {
                let (mut manifest, mut problem) = match PluginManifest::read(&dir) {
                    Ok(m) => (Some(Arc::new(m)), None),
                    Err(e) => (None, Some(e.0)),
                };
                if let Some(m) = &manifest
                    && reg.all.iter().any(|p| p.id() == m.id)
                {
                    problem = Some(format!("outro plugin já usa o id \"{}\"", m.id));
                    manifest = None;
                }
                let old = previous.remove(&dir);
                let reused = old.is_some();
                let mut entry = old.unwrap_or_else(|| Entry::new(dir, linked));
                if reused {
                    let old_id = entry.manifest.as_ref().map(|m| m.id.clone());
                    let old_main = entry.manifest.as_ref().and_then(|m| m.main.clone());
                    let new_id = manifest.as_ref().map(|m| m.id.clone());
                    let new_main = manifest.as_ref().and_then(|m| m.main.clone());
                    if old_id != new_id || old_main != new_main {
                        stops.push(detach(&mut reg, &mut entry));
                    }
                }
                entry.manifest = manifest;
                entry.problem = problem;
                entry.enabled = !reg.disabled.contains(&entry.id());
                let warnings = entry.manifest.as_ref().map(|m| m.warnings.clone()).unwrap_or_default();
                for w in warnings {
                    entry.note(&format!("aviso: {w}"));
                }
                reg.all.push(entry);
            }
            for (_, mut gone) in previous {
                stops.push(detach(&mut reg, &mut gone));
            }
            load_themes(&mut reg);
        }
        for (id, conn) in stops.into_iter().flatten() {
            let me = self.clone();
            self.spawn(async move {
                conn.close(SHUTDOWN_GRACE).await;
                me.note(&id, "processo parado");
            });
        }
        self.changed();
    }

    pub(crate) fn reload_themes(&self) {
        load_themes(&mut self.reg());
    }
}

fn detach(reg: &mut Registry, e: &mut Entry) -> Option<(String, Arc<crate::process::Connection>)> {
    e.starting = false;
    e.generation += 1;
    reg.unbusy(&e.id());
    e.conn.take().map(|c| (e.id(), c))
}

pub(crate) fn load_themes(reg: &mut Registry) {
    let mut themes = Themes::new();
    for e in reg.all.iter_mut() {
        e.palettes.clear();
        if !e.active() {
            continue;
        }
        let paths = e.manifest.as_ref().map(|m| m.themes.clone()).unwrap_or_default();
        for path in paths {
            let outcome = fs::read_to_string(e.dir.join(&path))
                .map_err(|err| err.to_string())
                .and_then(|text| Palette::from_json_str(&text).map_err(|err| err.to_string()))
                .and_then(|palette| themes.register(palette).map_err(|err| err.to_string()));
            match outcome {
                Ok(theme) => e.palettes.push(theme.id.clone()),
                Err(why) => e.note(&format!("tema {path}: {why}")),
            }
        }
    }
    reg.themes = themes;
}
