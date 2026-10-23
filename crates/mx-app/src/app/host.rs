use std::sync::Arc;
use std::time::Duration;

use gpui::{App, AppContext, Context, Entity, EventEmitter, Task};
use mx_core::ports::SystemClock;
use mx_core::text::Time;
use mx_store::{Changes, Effect, Ports, Store, StoreMsg, StoreOptions};
use tokio::runtime::Runtime;

use crate::app::ports::{Hooks, SysNotifier};
use mx_term::TermRegistry;

pub struct StoreChanged(pub Changes, pub Vec<Effect>);

pub struct StoreHost {
    pub store: Store,
    pub terms: TermRegistry,
    timer: Option<Task<()>>,
    armed: Option<Time>,
    rt: Arc<Runtime>,
}

impl EventEmitter<StoreChanged> for StoreHost {}

impl StoreHost {
    pub fn boot(opts: StoreOptions, cx: &mut App) -> anyhow::Result<Entity<StoreHost>> {
        let rt = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_name("mx-store")
                .enable_all()
                .build()?,
        );
        let (wake_tx, wake_rx) = async_channel::unbounded::<()>();
        let (events_tx, mut events_rx) = tokio::sync::mpsc::unbounded_channel();
        let state_dir = opts.state_dir.clone();
        let server = rt.block_on(async {
            std::fs::create_dir_all(&state_dir).ok();
            mx_claude::HookServer::start(&state_dir, events_tx).await
        });
        let server = server
            .map_err(|err| log::error!("hook server failed to start: {err}"))
            .ok();
        let plugins = mx_plugins::Plugins::new(&state_dir);
        plugins.set_runtime(rt.handle().clone());
        let terms = TermRegistry::new(mx_theme::TerminalPalette::default());
        let hooks = Arc::new(Hooks {
            home: opts.home.clone(),
            server,
        });
        let ports = Ports {
            clock: Arc::new(SystemClock),
            terms: Arc::new(terms.clone()),
            runner: Arc::new(mx_sys::LoginShell),
            notifier: Arc::new(SysNotifier::new(rt.handle().clone())),
            plugins: Arc::new(plugins.clone()),
            hooks: hooks.clone(),
            rt: rt.handle().clone(),
        };
        let mut store = Store::new(opts, ports);
        store.set_waker(Arc::new(move || {
            let _ = wake_tx.try_send(());
        }));
        if let Some(server) = &hooks.server {
            server.set_sink(store.sink());
        }
        let sender = store.sender();
        let exits = sender.clone();
        terms.on_exit(Arc::new(move |tab_id, code| {
            exits.send(StoreMsg::TermExited {
                tab_id: tab_id.to_owned(),
                code,
            })
        }));
        rt.spawn(async move {
            while let Some(ev) = events_rx.recv().await {
                sender.send(StoreMsg::Server(ev));
            }
        });
        store.load();
        store.attach_plugins(plugins);
        store.start();
        store.start_plugins();

        Ok(cx.new(|cx| {
            cx.spawn(async move |this, cx| {
                while wake_rx.recv().await.is_ok() {
                    while wake_rx.try_recv().is_ok() {}
                    let alive = this.update(cx, |h: &mut StoreHost, cx| {
                        h.store.pump();
                        h.flush(cx);
                    });
                    if alive.is_err() {
                        break;
                    }
                }
            })
            .detach();
            let mut host = StoreHost {
                store,
                terms,
                timer: None,
                armed: None,
                rt,
            };
            host.arm(cx);
            host
        }))
    }

    pub fn rt(&self) -> tokio::runtime::Handle {
        self.rt.handle().clone()
    }

    pub fn act<R>(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Store) -> R) -> R {
        let r = f(&mut self.store);
        self.flush(cx);
        r
    }

    pub fn flush(&mut self, cx: &mut Context<Self>) {
        let effects = self.store.take_effects();
        if self.store.has_changes() || !effects.is_empty() {
            let changes = self.store.take_changes();
            cx.emit(StoreChanged(changes, effects));
        }
        self.arm(cx);
    }

    fn arm(&mut self, cx: &mut Context<Self>) {
        let next = self.store.next_deadline();
        if next == self.armed && self.timer.is_some() {
            return;
        }
        self.armed = next;
        self.timer = next.map(|at| {
            let wait = (at - chrono::Local::now())
                .to_std()
                .unwrap_or(Duration::ZERO);
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(wait).await;
                let _ = this.update(cx, |h: &mut StoreHost, cx| {
                    h.armed = None;
                    h.timer = None;
                    h.store.fire_due();
                    h.flush(cx);
                });
            })
        });
    }
}

pub fn update_store<R>(
    host: &Entity<StoreHost>,
    cx: &mut App,
    f: impl FnOnce(&mut Store) -> R,
) -> R {
    host.update(cx, |h, cx| h.act(cx, f))
}
