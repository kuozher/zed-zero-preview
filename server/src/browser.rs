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
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        Command::new("cmd.exe")
            .raw_arg(format!("/c start \"\" \"{}\"", url))
            .spawn()
            .ok();
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(url).spawn().ok();
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
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
