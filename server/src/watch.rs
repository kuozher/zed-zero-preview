use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::io::Write;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

static CLIENT_ID: AtomicU64 = AtomicU64::new(1);

struct WatchEntry {
    clients: Vec<(u64, Arc<Mutex<TcpStream>>)>,
    watcher: Option<RecommendedWatcher>,
    notify_generation: u64,
    pending_filename: Option<String>,
}

#[derive(Clone)]
pub struct WatchRegistry {
    inner: Arc<Mutex<HashMap<PathBuf, WatchEntry>>>,
}

impl WatchRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn client_count(&self, watch_dir: &Path) -> usize {
        self.inner
            .lock()
            .unwrap()
            .get(watch_dir)
            .map(|e| e.clients.len())
            .unwrap_or(0)
    }

    pub fn add_client(&self, watch_dir: PathBuf, stream: TcpStream) {
        let client_id = CLIENT_ID.fetch_add(1, Ordering::SeqCst);
        let client = Arc::new(Mutex::new(stream));

        let mut map = self.inner.lock().unwrap();
        if !map.contains_key(&watch_dir) {
            self.create_entry(&mut map, watch_dir.clone());
        }
        map.get_mut(&watch_dir)
            .unwrap()
            .clients
            .push((client_id, client));
    }

    fn create_entry(&self, map: &mut HashMap<PathBuf, WatchEntry>, watch_dir: PathBuf) {
        let registry = self.clone();
        let dir = watch_dir.clone();

        let watcher_result = RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    if matches!(event.kind, EventKind::Remove(_)) {
                        return;
                    }
                    let filename = event
                        .paths
                        .last()
                        .and_then(|p| p.file_name())
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();

                    if should_ignore(&filename, &event.paths) {
                        return;
                    }

                    registry.schedule_notify(dir.clone(), filename);
                }
            },
            Config::default(),
        );

        let mut entry = WatchEntry {
            clients: Vec::new(),
            watcher: None,
            notify_generation: 0,
            pending_filename: None,
        };

        if let Ok(mut watcher) = watcher_result {
            if watcher.watch(&watch_dir, RecursiveMode::Recursive).is_ok() {
                entry.watcher = Some(watcher);
            }
        }

        map.insert(watch_dir, entry);
    }

    fn schedule_notify(&self, watch_dir: PathBuf, filename: String) {
        let generation = {
            let mut map = self.inner.lock().unwrap();
            let entry = match map.get_mut(&watch_dir) {
                Some(e) => e,
                None => return,
            };
            entry.pending_filename = Some(filename);
            entry.notify_generation += 1;
            entry.notify_generation
        };

        let registry = self.clone();
        let dir = watch_dir.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(120));
            registry.fire_if_current(dir, generation);
        });
    }

    fn fire_if_current(&self, watch_dir: PathBuf, generation: u64) {
        let filename = {
            let mut map = self.inner.lock().unwrap();
            let entry = match map.get_mut(&watch_dir) {
                Some(e) => e,
                None => return,
            };
            if entry.notify_generation != generation {
                return;
            }
            entry.pending_filename.clone().unwrap_or_default()
        };

        let is_css = filename.ends_with(".css");
        let msg = if is_css {
            format!("event: css\ndata: {}\n\n", filename)
        } else {
            "data: reload\n\n".to_string()
        };

        let mut alive_ids = Vec::new();
        {
            let map = self.inner.lock().unwrap();
            if let Some(entry) = map.get(&watch_dir) {
                for (id, client) in &entry.clients {
                    let ok = client
                        .lock()
                        .map(|mut s| {
                            s.write_all(msg.as_bytes())
                                .and_then(|_| s.flush())
                                .is_ok()
                        })
                        .unwrap_or(false);
                    if ok {
                        alive_ids.push(*id);
                    }
                }
            }
        }

        let mut map = self.inner.lock().unwrap();
        if let Some(entry) = map.get_mut(&watch_dir) {
            entry.clients.retain(|(id, _)| alive_ids.contains(id));
            if entry.clients.is_empty() {
                entry.watcher = None;
                map.remove(&watch_dir);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Read;
    use std::net::TcpListener;
    use std::time::Duration;

    #[test]
    fn registry_notifies_on_file_write() {
        let dir = std::env::temp_dir().join("zero-preview-watch-unit");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("page.html");
        fs::write(&file, "v1").unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let registry = WatchRegistry::new();
        let dir_for_server = dir.clone();

        let server_thread = thread::spawn(move || {
            let stream = listener.accept().unwrap().0;
            registry.add_client(dir_for_server, stream);
        });

        thread::sleep(Duration::from_millis(50));
        let mut client = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        server_thread.join().unwrap();

        thread::sleep(Duration::from_millis(150));
        fs::write(&file, "v2").unwrap();

        client
            .set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();

        let mut buf = [0u8; 1024];
        let mut received = String::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            if let Ok(n) = client.read(&mut buf) {
                if n > 0 {
                    received.push_str(&String::from_utf8_lossy(&buf[..n]));
                    if received.contains("data: reload") {
                        break;
                    }
                }
            }
            thread::sleep(Duration::from_millis(20));
        }

        assert!(
            received.contains("data: reload"),
            "got: {}",
            received
        );

        let _ = fs::remove_dir_all(&dir);
    }
}

fn should_ignore(filename: &str, paths: &[PathBuf]) -> bool {
    if !filename.is_empty()
        && (filename.contains("node_modules")
            || filename.contains(".git")
            || filename.ends_with(".tmp")
            || filename.starts_with('.'))
    {
        return true;
    }
    for p in paths {
        let s = p.to_string_lossy();
        if s.contains("node_modules") || s.contains(".git") {
            return true;
        }
    }
    false
}
