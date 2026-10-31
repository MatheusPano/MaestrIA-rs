use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::process::Command;

use super::Plugins;
use crate::MANIFEST_NAME;
use crate::catalog::PluginOrigin;
use crate::manifest::PluginManifest;
use crate::state::PluginInfo;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallError(pub String);

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for InstallError {}

fn err(message: impl Into<String>) -> InstallError {
    InstallError(message.into())
}

#[derive(Debug, Clone)]
pub struct StagedPlugin {
    pub staging: PathBuf,
    pub dir: PathBuf,
    pub manifest: Arc<PluginManifest>,
    pub source: String,
    pub origin: Option<PluginOrigin>,
    pub replacing: Option<String>,
}

fn is_git(source: &str) -> bool {
    ["https://", "http://", "git@", "ssh://"].iter().any(|p| source.starts_with(p)) || source.ends_with(".git")
}

fn clean_source(source: &str) -> String {
    mx_core::paths::expand_home(source.trim()).trim_end_matches('/').to_owned()
}

fn rm(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

fn manifest_dir(dir: &Path) -> Option<PathBuf> {
    if dir.join(MANIFEST_NAME).is_file() {
        return Some(dir.to_path_buf());
    }
    let children: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            !name.starts_with('.') && name != "__MACOSX" && e.path().is_dir()
        })
        .map(|e| e.path())
        .collect();
    match children.as_slice() {
        [only] if only.join(MANIFEST_NAME).is_file() => Some(only.clone()),
        _ => None,
    }
}

impl Plugins {
    async fn run_tool(&self, program: &str, args: &[&str]) -> Result<(), InstallError> {
        let env = self.inner.env.read().unwrap().clone();
        let out = Command::new(program)
            .args(args)
            .env_clear()
            .envs(env)
            .stdin(std::process::Stdio::null())
            .output()
            .await
            .map_err(|e| err(e.to_string()))?;
        if out.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&out.stderr);
        let why = stderr.split('\n').rfind(|l| !l.trim().is_empty()).map(str::to_owned);
        Err(err(why.unwrap_or_else(|| format!("Falhou (código {})", out.status.code().unwrap_or(-1)))))
    }

    pub async fn stage(&self, source: &str) -> Result<StagedPlugin, InstallError> {
        self.stage_with(source, None, None).await
    }

    pub async fn stage_with(
        &self,
        source: &str,
        origin: Option<PluginOrigin>,
        git_ref: Option<&str>,
    ) -> Result<StagedPlugin, InstallError> {
        let src = clean_source(source);
        if src.is_empty() {
            return Err(err("Diga de onde instalar"));
        }
        let base = self.inner.root.join(".staging");
        fs::create_dir_all(&base).map_err(|e| err(e.to_string()))?;
        let staging = tempfile::Builder::new()
            .prefix("p")
            .tempdir_in(&base)
            .map_err(|e| err(e.to_string()))?
            .keep();
        let outcome = self.stage_into(source, &src, &staging, origin, git_ref).await;
        if outcome.is_err() {
            rm(&staging);
        }
        outcome
    }

    async fn stage_into(
        &self,
        source: &str,
        src: &str,
        staging: &Path,
        origin: Option<PluginOrigin>,
        git_ref: Option<&str>,
    ) -> Result<StagedPlugin, InstallError> {
        let into = staging.join("src");
        let into_s = into.to_string_lossy().into_owned();
        let git = is_git(src);
        if git {
            let mut args = vec!["clone", "--depth", "1"];
            if let Some(r) = git_ref {
                args.extend(["--branch", r]);
            }
            args.extend([src, into_s.as_str()]);
            self.run_tool("git", &args).await?;
        } else if src.to_lowercase().ends_with(".zip") && Path::new(src).is_file() {
            if cfg!(target_os = "macos") {
                self.run_tool("ditto", &["-x", "-k", src, &into_s]).await?;
            } else {
                self.run_tool("unzip", &["-q", src, "-d", &into_s]).await?;
            }
        } else if Path::new(src).is_dir() {
            self.run_tool("cp", &["-R", src, &into_s]).await?;
        } else {
            return Err(err(format!("\"{source}\" não é uma URL de git, um .zip nem uma pasta que exista")));
        }
        let dir = manifest_dir(&into).ok_or_else(|| err(format!("Não achei um {MANIFEST_NAME} ali")))?;
        let manifest = PluginManifest::read(&dir).map_err(|e| err(e.0))?;
        let replacing = self.get(&manifest.id).and_then(|p| p.manifest).map(|m| m.version.clone());
        Ok(StagedPlugin {
            staging: staging.to_path_buf(),
            dir,
            manifest: Arc::new(manifest),
            source: source.trim().to_owned(),
            origin: origin.or_else(|| git.then(|| PluginOrigin::Git { url: src.to_owned(), git_ref: git_ref.map(str::to_owned) })),
            replacing,
        })
    }

    pub fn discard(&self, staged: &StagedPlugin) {
        rm(&staged.staging);
    }

    async fn remove(&self, p: &PluginInfo) {
        self.stop(&p.id).await;
        let is_link = fs::symlink_metadata(&p.dir).is_ok_and(|m| m.file_type().is_symlink());
        if is_link {
            let _ = fs::remove_file(&p.dir);
        } else {
            rm(&p.dir);
        }
    }

    pub async fn commit(&self, staged: &StagedPlugin) -> Result<PluginInfo, InstallError> {
        let id = staged.manifest.id.clone();
        let moved = async {
            fs::create_dir_all(&self.inner.root).map_err(|e| err(e.to_string()))?;
            let old = self.get(&id);
            if let Some(old) = &old {
                self.stop(&old.id).await;
            }
            let aside = staged.staging.join("old");
            let parked = match &old {
                Some(old) if fs::symlink_metadata(&old.dir).is_ok() => {
                    fs::rename(&old.dir, &aside).map_err(|e| err(e.to_string()))?;
                    Some(old.dir.clone())
                }
                _ => None,
            };
            fs::rename(&staged.dir, self.inner.root.join(&id)).map_err(|e| {
                if let Some(dir) = &parked {
                    let _ = fs::rename(&aside, dir);
                }
                err(e.to_string())
            })
        }
        .await;
        rm(&staged.staging);
        moved?;
        match &staged.origin {
            Some(o) => drop(self.reg().origins.insert(id.clone(), o.clone())),
            None => drop(self.reg().origins.remove(&id)),
        }
        self.scan();
        let plugin = self.get(&id).ok_or_else(|| err(format!("Não achei um {MANIFEST_NAME} ali")))?;
        self.note(&id, &format!("instalado de {}", staged.source));
        self.start_on_install(&plugin);
        Ok(plugin)
    }

    fn start_on_install(&self, plugin: &PluginInfo) {
        if plugin.active() && plugin.manifest.as_ref().is_some_and(|m| m.activates_on("onStartup")) {
            self.start_detached(&plugin.id);
        }
    }

    pub async fn link(&self, path: &str) -> Result<PluginInfo, InstallError> {
        let src = clean_source(path);
        let src_path = Path::new(&src);
        if !src_path.is_dir() {
            return Err(err(format!("\"{path}\" não é uma pasta")));
        }
        let manifest = PluginManifest::read(src_path).map_err(|e| err(e.0))?;
        if self.get(&manifest.id).is_some() {
            return Err(err(format!("Já existe um plugin \"{}\" — remova ele antes", manifest.id)));
        }
        fs::create_dir_all(&self.inner.root).map_err(|e| err(e.to_string()))?;
        let absolute = std::path::absolute(src_path).map_err(|e| err(e.to_string()))?;
        std::os::unix::fs::symlink(&absolute, self.inner.root.join(&manifest.id))
            .map_err(|e| err(e.to_string()))?;
        self.reg().origins.remove(&manifest.id);
        self.scan();
        let plugin = self.get(&manifest.id).ok_or_else(|| err(format!("\"{path}\" não é uma pasta")))?;
        self.note(&plugin.id, &format!("carregado de {src} (desenvolvimento)"));
        self.start_on_install(&plugin);
        Ok(plugin)
    }

    pub async fn uninstall(&self, id: &str) {
        let Some(p) = self.get(id) else { return };
        self.remove(&p).await;
        rm(&self.data_dir(id));
        {
            let mut reg = self.reg();
            reg.disabled.remove(id);
            reg.origins.remove(id);
            reg.manual_update.remove(id);
        }
        self.scan();
    }
}

pub fn trust_headline(m: &PluginManifest, replacing: Option<&str>) -> String {
    let mut parts = vec![format!("{} {}", m.id, m.version)];
    if !m.author.is_empty() {
        parts.push(format!("de {}", m.author));
    }
    if let Some(v) = replacing {
        parts.push(format!("no lugar da {v}"));
    }
    parts.join(" · ")
}

pub fn trust_facts(m: &PluginManifest, linux: bool) -> Vec<String> {
    let mut facts = vec![match &m.main {
        Some(main) if m.has_process() => format!("Roda um programa seu, com as suas permissões: {}", main.join(" ")),
        _ => "Só declarações — nenhum programa roda".to_owned(),
    }];
    facts.extend(m.permissions.iter().map(|p| format!("Pode {}", p.label())));
    if !m.commands.is_empty() {
        let list: Vec<String> = m
            .commands
            .iter()
            .map(|c| match c.key {
                Some(k) => format!("{} ({})", c.title, k.label(linux)),
                None => c.title.clone(),
            })
            .collect();
        facts.push(format!("Comandos: {}", list.join(", ")));
    }
    if !m.tools.is_empty() {
        let names: Vec<&str> = m.tools.iter().map(|t| t.name.as_str()).collect();
        facts.push(format!("Ferramentas para o Claude, liberadas em toda sessão: {}", names.join(", ")));
    }
    match m.themes.len() {
        0 => {}
        1 => facts.push("Um tema".into()),
        n => facts.push(format!("{n} temas")),
    }
    facts.extend(m.warnings.iter().cloned());
    facts
}

pub fn installed_banner(name: &str, version: &str, replacing: Option<&str>) -> String {
    match replacing {
        None => format!("{name} {version} instalado"),
        Some(old) => format!("{name} atualizado de {old} pra {version}"),
    }
}
