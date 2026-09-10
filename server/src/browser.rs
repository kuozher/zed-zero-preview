use std::process::Command;
use std::sync::Mutex;

pub trait BrowserOpener: Send + Sync {
    fn open(&self, url: &str);
}

pub struct DefaultBrowser;

impl BrowserOpener for DefaultBrowser {
    fn open(&self, url: &str) {
        open_browser(url);
    }
}

pub fn open_browser(url: &str) {
    if cfg!(windows) {
        Command::new("cmd.exe")
            .args(["/c", &format!("start \"\" \"{}\"", url)])
            .spawn()
            .ok();
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(url).spawn().ok();
    } else {
        Command::new("xdg-open").arg(url).spawn().ok();
    }
}

pub struct MockBrowser {
    pub urls: Mutex<Vec<String>>,
}

impl MockBrowser {
    pub fn new() -> Self {
        Self {
            urls: Mutex::new(Vec::new()),
        }
    }
}

impl BrowserOpener for MockBrowser {
    fn open(&self, url: &str) {
        self.urls.lock().unwrap().push(url.to_string());
    }
}
