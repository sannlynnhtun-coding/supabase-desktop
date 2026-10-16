use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::os::windows::{fs::OpenOptionsExt, process::CommandExt};
use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub state: String,
    pub message: String,
    pub studio_url: Option<String>,
    #[serde(default)]
    pub available: Vec<String>,
    #[serde(default)]
    pub unavailable: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Restart,
    Exit,
}
#[derive(Default)]
pub struct Confirmation {
    pending: Option<Action>,
    in_flight: bool,
}
impl Confirmation {
    pub fn request(&mut self, action: Action) -> bool {
        if self.pending.is_some() || self.in_flight {
            return false;
        }
        self.pending = Some(action);
        true
    }
    pub fn cancel(&mut self) {
        self.pending = None;
    }
    pub fn confirm(&mut self, action: Action) -> Option<Action> {
        if self.pending != Some(action) {
            return None;
        }
        self.pending = None;
        self.in_flight = true;
        Some(action)
    }
    pub fn finished(&mut self) {
        self.in_flight = false;
    }
}
impl Action {
    pub fn title(self) -> &'static str {
        match self {
            Self::Restart => "Confirm restart",
            Self::Exit => "Confirm exit",
        }
    }
    pub fn button(self) -> &'static str {
        match self {
            Self::Restart => "Restart services",
            Self::Exit => "Exit Supabase Desktop",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Restart => {
                "Restart all local Supabase services? Active connections will be interrupted. Your project data will be kept."
            }
            Self::Exit => {
                "Exit Supabase Desktop and stop all local services? Your project data will be kept."
            }
        }
    }
}
pub enum Event {
    Status(Status),
    Log(String),
    Error(String),
    Stopped,
}

pub struct Session {
    pub commands: Sender<Action>,
    pub events: Receiver<Event>,
}

struct Manager {
    child: Option<Child>,
    token: String,
    directory: PathBuf,
    data: PathBuf,
    http: ureq::Agent,
    _lock: File,
}

impl Manager {
    fn new(directory: PathBuf, data: PathBuf) -> Result<Self> {
        let root =
            PathBuf::from(std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA unavailable")?)
                .join("SupabaseLocal");
        std::fs::create_dir_all(&root)?;
        let lock = OpenOptions::new().create(true).truncate(false).read(true).write(true).share_mode(0)
            .open(root.join("launcher.lock")).context("Supabase Desktop is already running. Use its notification icon, or close it before opening another launcher.")?;
        let mut random = [0u8; 32];
        getrandom::fill(&mut random)
            .map_err(|e| anyhow::anyhow!("Could not generate launcher token: {e}"))?;
        let token = random.iter().map(|b| format!("{b:02x}")).collect();
        let config = ureq::Agent::config_builder()
            .proxy(None)
            .max_redirects(0)
            .timeout_global(Some(Duration::from_secs(3)))
            .build();
        Ok(Self {
            child: None,
            directory,
            data,
            token,
            http: config.into(),
            _lock: lock,
        })
    }
    fn start(&mut self) -> Result<()> {
        ensure!(self.child.is_none(), "A previous manager has not stopped");
        let address = "127.0.0.1:54320".parse().unwrap();
        ensure!(
            std::net::TcpStream::connect_timeout(&address, Duration::from_millis(300)).is_err(),
            "The local manager port is already in use. Close the other Supabase Desktop instance first."
        );
        std::fs::create_dir_all(&self.data)?;
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.data.join("desktop-manager.log"))?;
        self.child = Some(
            Command::new(self.directory.join("SupabaseLocal.exe"))
                .creation_flags(0x08000000)
                .current_dir(&self.directory)
                .env("LauncherToken", &self.token)
                .env("RuntimeRoot", self.directory.join("runtime"))
                .env("DataRoot", &self.data)
                .stdout(Stdio::from(log.try_clone()?))
                .stderr(Stdio::from(log))
                .spawn()
                .context("Could not start the installed service manager")?,
        );
        Ok(())
    }
    fn status(&self) -> Result<Status> {
        Ok(self
            .http
            .get("http://127.0.0.1:54320/api/status")
            .call()?
            .body_mut()
            .read_json()?)
    }
    fn stop(&mut self) -> Result<()> {
        let Some(child) = self.child.as_mut() else {
            return Ok(());
        };
        if child.try_wait()?.is_none() {
            self.http
                .post("http://127.0.0.1:54320/api/shutdown")
                .header("X-Launcher-Token", &self.token)
                .send_empty()
                .context(
                    "Graceful shutdown request failed; services are still owned by this launcher",
                )?;
            let deadline = Instant::now() + Duration::from_secs(100);
            while child.try_wait()?.is_none() {
                ensure!(
                    Instant::now() < deadline,
                    "Services did not stop in time. Keep the launcher open and check its activity log."
                );
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        self.child = None;
        Ok(())
    }
}

impl Drop for Manager {
    fn drop(&mut self) {
        if self.child.is_some() {
            let _ = self.stop();
        }
    }
}

pub fn start_session(directory: PathBuf, data: PathBuf) -> Session {
    let (commands, receiver) = mpsc::channel();
    let (events, output) = mpsc::channel();
    std::thread::spawn(move || {
        let run = || -> Result<()> {
            let mut manager = Manager::new(directory, data)?;
            let _ = events.send(Event::Log("Starting private native services…".into()));
            manager.start()?;
            let mut last = String::new();
            let mut started = Instant::now();
            let mut timeout_reported = false;
            loop {
                match receiver.recv_timeout(Duration::from_millis(700)) {
                    Ok(action) => {
                        let _ = events.send(Event::Log("Stopping services gracefully…".into()));
                        match manager.stop() {
                            Ok(()) if action == Action::Exit => {
                                let _ = events.send(Event::Stopped);
                                return Ok(());
                            }
                            Ok(()) => {
                                let _ = events.send(Event::Log(
                                    "Previous services stopped. Restarting…".into(),
                                ));
                                manager.start()?;
                                started = Instant::now();
                                timeout_reported = false;
                                last.clear();
                            }
                            Err(error) => {
                                let _ = events.send(Event::Error(format!("{error:#}")));
                            }
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        // The UI normally stays alive until Stopped. Still request a
                        // graceful shutdown if its event receiver disappears.
                        manager.stop()?;
                        return Ok(());
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                if let Some(child) = manager.child.as_mut() {
                    if let Some(code) = child.try_wait()? {
                        manager.child = None;
                        let _ = events.send(Event::Error(format!("Service manager exited ({code}). Check desktop-manager.log in the data folder.")));
                    }
                }
                if manager.child.is_none() {
                    continue;
                }
                if let Ok(status) = manager.status() {
                    let serialized = serde_json::to_string(&status)?;
                    if serialized != last {
                        let _ = events.send(Event::Log(status.message.clone()));
                        let _ = events.send(Event::Status(status));
                        last = serialized;
                    }
                } else if started.elapsed() > Duration::from_secs(300) && !timeout_reported {
                    let _ = events.send(Event::Error(
                        "Startup timed out. Check the activity log and conflicting local ports."
                            .into(),
                    ));
                    timeout_reported = true;
                }
            }
        };
        if let Err(error) = run() {
            let _ = events.send(Event::Error(format!("{error:#}")));
        }
    });
    Session {
        commands,
        events: output,
    }
}

pub fn installation_directory() -> Result<PathBuf> {
    Ok(std::env::current_exe()?
        .parent()
        .context("Missing application directory")?
        .to_path_buf())
}
pub fn default_data() -> Result<PathBuf> {
    Ok(
        PathBuf::from(std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA unavailable")?)
            .join("SupabaseLocal")
            .join("data"),
    )
}
pub fn open_url(url: &str) -> Result<()> {
    ensure!(
        ["http://127.0.0.1:54323", "http://127.0.0.1:54320"].contains(&url),
        "Unexpected local application URL"
    );
    #[link(name = "shell32")]
    unsafe extern "system" {
        fn ShellExecuteW(
            owner: isize,
            verb: *const u16,
            file: *const u16,
            params: *const u16,
            directory: *const u16,
            show: i32,
        ) -> isize;
    }
    let verb: Vec<u16> = "open\0".encode_utf16().collect();
    let file: Vec<u16> = format!("{url}\0").encode_utf16().collect();
    let result = unsafe {
        ShellExecuteW(
            0,
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    ensure!(result > 32, "Could not open the local URL");
    Ok(())
}

pub fn verify_lifecycle(directory: &Path, data: &Path, restart: bool) -> Result<()> {
    let mut manager = Manager::new(directory.to_path_buf(), data.to_path_buf())?;
    let cycles = if restart { 2 } else { 1 };
    let mut ids = Vec::new();
    let result = (|| -> Result<()> {
        for _ in 0..cycles {
            manager.start()?;
            ids.push(manager.child.as_ref().unwrap().id());
            let deadline = Instant::now() + Duration::from_secs(300);
            loop {
                ensure!(
                    manager.child.as_mut().unwrap().try_wait()?.is_none(),
                    "Service manager exited before readiness; see desktop-manager.log"
                );
                if let Ok(status) = manager.status() {
                    ensure!(status.state != "failed", "{}", status.message);
                    if status.studio_url.is_some() && status.available.len() == 6 {
                        std::fs::write(
                            data.join("launcher-verification.json"),
                            serde_json::to_vec(&status)?,
                        )?;
                        break;
                    }
                }
                ensure!(Instant::now() < deadline, "Launcher readiness timed out");
                std::thread::sleep(Duration::from_millis(500));
            }
            manager.stop()?;
        }
        ensure!(
            cycles == 1 || ids[0] != ids[1],
            "Restart reused the old manager"
        );
        std::fs::write(
            data.join("rust-lifecycle-result.json"),
            serde_json::to_vec(
                &serde_json::json!({"cycles":cycles,"managerIds":ids,"gracefulShutdown":true}),
            )?,
        )?;
        Ok(())
    })();
    if manager.child.is_some() {
        manager.stop()?;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canceled_or_unconfirmed_actions_never_dispatch_and_confirm_only_once() {
        let mut gate = Confirmation::default();
        assert_eq!(gate.confirm(Action::Exit), None);
        assert!(gate.request(Action::Exit));
        assert_eq!(gate.confirm(Action::Restart), None);
        gate.cancel();
        assert_eq!(gate.confirm(Action::Exit), None);
        assert!(gate.request(Action::Restart));
        assert_eq!(gate.confirm(Action::Restart), Some(Action::Restart));
        assert_eq!(gate.confirm(Action::Restart), None);
        assert!(!gate.request(Action::Exit));
        gate.finished();
        assert!(gate.request(Action::Exit));
    }
    #[test]
    fn restart_and_exit_have_explicit_distinct_confirmations() {
        assert_eq!(Action::Restart.title(), "Confirm restart");
        assert_eq!(Action::Exit.title(), "Confirm exit");
        assert_ne!(Action::Restart.description(), Action::Exit.description());
    }
}
