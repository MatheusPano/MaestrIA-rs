use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use mx_term::{GridSize, Snapshot, SpawnOptions, TermEvent, TerminalSession};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args[1].clone();
    let cols: u16 = args[2].parse().unwrap();
    let rows: u16 = args[3].parse().unwrap();
    let render = args.get(4).is_some_and(|a| a == "render");
    let mb: f64 = std::env::var("MB").ok().and_then(|v| v.parse().ok()).unwrap_or(50.0);
    let opts = SpawnOptions { program: Some("/bin/sh".into()), args: vec!["-c".into(), cmd.clone()], ..Default::default() };
    let size = GridSize { cols, rows, cell_width: 8, cell_height: 16 };
    let t = Instant::now();
    let s = TerminalSession::spawn(opts, size, Default::default()).unwrap();
    let done = Arc::new(AtomicBool::new(false));
    let r = render.then(|| {
        let (s2, d2) = (s.clone(), done.clone());
        std::thread::spawn(move || {
            let mut snap = Snapshot::default();
            let mut frames = 0;
            while !d2.load(Ordering::Relaxed) {
                if s2.take_dirty() {
                    s2.snapshot_into(&mut snap);
                    frames += 1;
                }
                std::thread::sleep(Duration::from_millis(8));
            }
            frames
        })
    });
    let ev = s.events();
    while let Ok(e) = ev.recv_blocking() {
        if matches!(e, TermEvent::Exited(_)) {
            break;
        }
    }
    let dt = t.elapsed().as_secs_f64();
    done.store(true, Ordering::Relaxed);
    let frames = r.map(|h| h.join().unwrap()).unwrap_or(0);
    println!("pty {cols}x{rows} render={render}: {:.1} MB/s ({frames} frames) [{cmd}]", mb / dt);
}
