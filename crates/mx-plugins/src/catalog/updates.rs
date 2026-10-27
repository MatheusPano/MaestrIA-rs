use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use std::time::Duration;

use chrono::{DateTime, Local};
use mx_core::version::compare_versions;

use super::fetch::{download, get, git_head, sha256_file};
use super::{CatalogEntry, PluginCatalog, PluginOrigin, PluginUpdate};
use crate::manifest::PluginManifest;
use crate::registry::{InstallError, Plugins, StagedPlugin};
use crate::state::PluginInfo;

pub const FIRST_DELAY: Duration = Duration::from_secs(45);
pub const EVERY: Duration = Duration::from_secs(6 * 60 * 60);

type Hook = Arc<dyn Fn() + Send + Sync>;
type InstalledHook = Arc<dyn Fn(&PluginInfo) + Send + Sync>;

#[derive(Default)]
struct State {
    loaded: HashMap<String, PluginCatalog>,
    available: BTreeMap<String, PluginUpdate>,
    offers: BTreeMap<String, PluginUpdate>,
    busy: BTreeSet<String>,
    git_seen: HashMap<String, (String, Option<PluginUpdate>)>,
    checked_at: Option<DateTime<Local>>,
    checking: bool,
    error: Option<String>,
}

struct Inner {
    plugins: Plugins,
    catalogs: Vec<String>,
    state: Mutex<State>,
    listener: RwLock<Option<Hook>>,
    installed: RwLock<Option<InstalledHook>>,
    timer: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

#[derive(Clone)]
pub struct PluginUpdates {
    inner: Arc<Inner>,
}

fn install_err(e: impl ToString) -> InstallError {
    InstallError(e.to_string())
}

impl PluginUpdates {
    pub fn new(plugins: Plugins, catalogs: Vec<String>) -> PluginUpdates {
        PluginUpdates {
            inner: Arc::new(Inner {
                plugins,
                catalogs,
                state: Mutex::new(State::default()),
                listener: RwLock::new(None),
                installed: RwLock::new(None),
                timer: Mutex::new(None),
            }),
        }
    }

    pub fn set_listener(&self, f: impl Fn() + Send + Sync + 'static) {
        *self.inner.listener.write().unwrap() = Some(Arc::new(f));
    }

    pub fn set_installed(&self, f: impl Fn(&PluginInfo) + Send + Sync + 'static) {
        *self.inner.installed.write().unwrap() = Some(Arc::new(f));
    }

    fn st(&self) -> MutexGuard<'_, State> {
        self.inner.state.lock().unwrap()
    }

    fn notify(&self) {
        let l = self.inner.listener.read().unwrap().clone();
        if let Some(l) = l {
            l();
        }
    }

    pub fn plugins(&self) -> &Plugins {
        &self.inner.plugins
    }

    pub fn catalogs(&self) -> &[String] {
        &self.inner.catalogs
    }

    pub fn loaded(&self, url: &str) -> Option<PluginCatalog> {
        self.st().loaded.get(url).cloned()
    }

    pub fn available(&self) -> BTreeMap<String, PluginUpdate> {
        self.st().available.clone()
    }

    pub fn offers(&self) -> BTreeMap<String, PluginUpdate> {
        self.st().offers.clone()
    }

    pub fn busy(&self) -> BTreeSet<String> {
        self.st().busy.clone()
    }

    pub fn is_busy(&self, id: &str) -> bool {
        self.st().busy.contains(id)
    }

    pub fn checked_at(&self) -> Option<DateTime<Local>> {
        self.st().checked_at
    }

    pub fn checking(&self) -> bool {
        self.st().checking
    }

    pub fn error(&self) -> Option<String> {
        self.st().error.clone()
    }

    pub fn start(&self) {
        self.start_with(FIRST_DELAY, EVERY);
    }

    pub fn start_with(&self, first: Duration, every: Duration) {
        self.stop();
        let me = self.clone();
        let work = async move {
            tokio::time::sleep(first).await;
            loop {
                me.check().await;
                tokio::time::sleep(every).await;
            }
        };
        let handle = self.inner.plugins.inner.runtime.read().unwrap().clone();
        let task = match handle {
            Some(h) => h.spawn(work),
            None => tokio::spawn(work),
        };
        *self.inner.timer.lock().unwrap() = Some(task);
    }

    pub fn stop(&self) {
        if let Some(t) = self.inner.timer.lock().unwrap().take() {
            t.abort();
        }
    }

    pub async fn load(&self, url: &str) -> Result<PluginCatalog, String> {
        let catalog = PluginCatalog::parse(url, &get(url).await?)?;
        self.st().loaded.insert(url.to_owned(), catalog.clone());
        self.notify();
        Ok(catalog)
    }

    pub async fn check(&self) {
        {
            let mut st = self.st();
            if st.checking {
                return;
            }
            st.checking = true;
            st.error = None;
        }
        self.notify();
        for url in self.inner.catalogs.clone() {
            if let Err(e) = self.load(&url).await {
                self.st().error = Some(format!("Não deu pra ler o catálogo: {e}"));
            }
        }
        let (mut found, mut offered) = (BTreeMap::new(), BTreeMap::new());
        for p in self.inner.plugins.all() {
            let Some(m) = p.manifest.clone() else { continue };
            if p.linked {
                continue;
            }
            match self.lookup(&p, &m).await {
                Ok(Some(u)) => drop(found.insert(p.id.clone(), u)),
                Ok(None) => {}
                Err(e) => self.inner.plugins.note(&p.id, &format!("procurar versão nova: {e}")),
            }
            if let Some(o) = self.offer(&p, &m) {
                offered.insert(p.id.clone(), o);
            }
        }
        let ready: Vec<PluginUpdate> = {
            let mut st = self.st();
            st.available = found;
            st.offers = offered;
            st.checked_at = Some(Local::now());
            st.checking = false;
            st.available.values().cloned().collect()
        };
        self.notify();
        for u in ready {
            if self.enters_alone(&u) {
                self.quietly(&u).await;
            }
        }
    }

    pub fn enters_alone(&self, u: &PluginUpdate) -> bool {
        let p = &self.inner.plugins;
        p.auto_update() && !p.manual_update().contains(&u.id) && !u.asks_more && u.newer()
    }

    async fn lookup(&self, p: &PluginInfo, m: &PluginManifest) -> Result<Option<PluginUpdate>, InstallError> {
        let Some(origin) = self.inner.plugins.origin_of(&p.id) else { return Ok(None) };
        if let Some(url) = origin.catalog() {
            let Some(e) = self.st().loaded.get(url).and_then(|c| c.by_id(&p.id)).cloned() else { return Ok(None) };
            if !e.fits() || compare_versions(&e.version, &m.version).is_le() {
                return Ok(None);
            }
            let asks_more = e.permissions.iter().any(|id| !m.permissions.iter().any(|perm| perm.id() == id));
            return Ok(Some(PluginUpdate {
                id: p.id.clone(),
                from: m.version.clone(),
                to: e.version.clone(),
                origin: origin.clone(),
                entry: Some(e),
                asks_more,
            }));
        }
        let Some((git, git_ref)) = origin.git() else { return Ok(None) };
        let env = self.inner.plugins.inner.env.read().unwrap().clone();
        let Some(head) = git_head(git, git_ref, &env).await else { return Ok(None) };
        if let Some((seen, u)) = self.st().git_seen.get(&p.id)
            && *seen == head
        {
            return Ok(u.clone());
        }
        let staged = self.inner.plugins.stage_with(git, None, git_ref).await?;
        let next = &staged.manifest;
        let u = (next.id == p.id && compare_versions(&next.version, &m.version).is_gt()).then(|| PluginUpdate {
            id: p.id.clone(),
            from: m.version.clone(),
            to: next.version.clone(),
            origin: origin.clone(),
            entry: None,
            asks_more: Self::asks_more(m, next),
        });
        self.st().git_seen.insert(p.id.clone(), (head, u.clone()));
        self.inner.plugins.discard(&staged);
        Ok(u)
    }

    fn offer(&self, p: &PluginInfo, m: &PluginManifest) -> Option<PluginUpdate> {
        if self.inner.plugins.origin_of(&p.id).is_some_and(|o| o.catalog().is_some()) {
            return None;
        }
        let st = self.st();
        self.inner.catalogs.iter().find_map(|url| {
            let e = st.loaded.get(url)?.by_id(&p.id)?;
            if !e.fits() || compare_versions(&e.version, &m.version).is_lt() {
                return None;
            }
            Some(PluginUpdate {
                id: p.id.clone(),
                from: m.version.clone(),
                to: e.version.clone(),
                origin: PluginOrigin::Catalog(url.clone()),
                entry: Some(e.clone()),
                asks_more: false,
            })
        })
    }

    pub fn asks_more(old: &PluginManifest, next: &PluginManifest) -> bool {
        next.permissions.iter().any(|p| !old.permissions.contains(p)) || (!old.has_process() && next.has_process())
    }

    pub async fn stage_entry(&self, e: &CatalogEntry, catalog: &str) -> Result<StagedPlugin, InstallError> {
        let origin = PluginOrigin::Catalog(catalog.to_owned());
        let plugins = &self.inner.plugins;
        if let Some(git) = &e.git {
            let staged = plugins.stage_with(git, Some(origin), e.git_ref.as_deref()).await?;
            return self.same_id(staged, &e.id);
        }
        let (Some(url), Some(want)) = (&e.url, &e.sha256) else {
            return Err(InstallError(format!("O pacote de {} não confere (sha256 )", e.name)));
        };
        let base = plugins.root().join(".staging");
        std::fs::create_dir_all(&base).map_err(install_err)?;
        let tmp = tempfile::Builder::new().prefix("dl").tempdir_in(&base).map_err(install_err)?;
        let file = tmp.path().join(format!("{}.zip", e.id));
        download(url, &file).await?;
        let got = sha256_file(&file)?;
        if got != *want {
            return Err(InstallError(format!("O pacote de {} não confere (sha256 {got})", e.name)));
        }
        let mut staged = plugins.stage_with(&file.to_string_lossy(), Some(origin), None).await?;
        staged.source = url.clone();
        drop(tmp);
        self.same_id(staged, &e.id)
    }

    fn same_id(&self, staged: StagedPlugin, id: &str) -> Result<StagedPlugin, InstallError> {
        if staged.manifest.id == id {
            return Ok(staged);
        }
        self.inner.plugins.discard(&staged);
        Err(InstallError(format!("O pacote é de \"{}\", e não de \"{id}\"", staged.manifest.id)))
    }

    pub async fn stage_update(&self, u: &PluginUpdate) -> Result<StagedPlugin, InstallError> {
        if let (Some(e), Some(catalog)) = (&u.entry, u.origin.catalog()) {
            return self.stage_entry(e, catalog).await;
        }
        match u.origin.git() {
            Some((git, git_ref)) => self.inner.plugins.stage_with(git, None, git_ref).await,
            None => Err(InstallError("Sem origem".into())),
        }
    }

    pub async fn finish(&self, staged: &StagedPlugin) -> Result<PluginInfo, InstallError> {
        let plugin = self.inner.plugins.commit(staged).await?;
        {
            let mut st = self.st();
            st.available.remove(&plugin.id);
            st.offers.remove(&plugin.id);
            st.git_seen.remove(&plugin.id);
        }
        self.notify();
        Ok(plugin)
    }

    pub async fn prepare(&self, u: &PluginUpdate) -> Result<StagedPlugin, InstallError> {
        if !self.st().busy.insert(u.id.clone()) {
            return Err(InstallError("Já está baixando".into()));
        }
        self.notify();
        let out = self.stage_update(u).await;
        self.st().busy.remove(&u.id);
        self.notify();
        out
    }

    async fn quietly(&self, u: &PluginUpdate) {
        let plugins = &self.inner.plugins;
        let Some(old) = plugins.get(&u.id).and_then(|p| p.manifest) else { return };
        if !self.st().busy.insert(u.id.clone()) {
            return;
        }
        self.notify();
        match self.stage_update(u).await {
            Ok(staged) if Self::asks_more(&old, &staged.manifest) => {
                plugins.discard(&staged);
                self.st().available.insert(u.id.clone(), PluginUpdate { asks_more: true, ..u.clone() });
            }
            Ok(staged) => match self.finish(&staged).await {
                Ok(info) => {
                    plugins.note(&info.id, &format!("atualizado sozinho de {} pra {}", u.from, u.to));
                    let hook = self.inner.installed.read().unwrap().clone();
                    if let Some(hook) = hook {
                        hook(&info);
                    }
                }
                Err(e) => plugins.note(&u.id, &format!("atualizar pra {}: {e}", u.to)),
            },
            Err(e) => plugins.note(&u.id, &format!("atualizar pra {}: {e}", u.to)),
        }
        self.st().busy.remove(&u.id);
        self.notify();
    }
}

pub async fn fetch_icon(url: &str) -> Option<Vec<u8>> {
    if !url.to_lowercase().ends_with(".svg") {
        return None;
    }
    super::fetch::get_bytes(url).await.ok()
}
