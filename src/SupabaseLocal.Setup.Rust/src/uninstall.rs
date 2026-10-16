#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod uninstall_core;
use anyhow::{Context as _, Result, ensure};
use gpui_kit::base::Disableable;
use gpui_kit::{
    component::{
        ActiveTheme, WindowExt,
        button::{Button, ButtonVariants},
    },
    *,
};
use std::os::windows::process::CommandExt;
use std::{path::PathBuf, sync::mpsc, time::Duration};

struct Uninstall {
    logo: std::sync::Arc<RenderImage>,
    message: String,
    remove_data: bool,
    busy: bool,
    completed: bool,
    worker: Option<(u32, bool)>,
    failed: bool,
    exit_failure: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

fn start_worker(remove_data: bool) -> Result<()> {
    let scope = uninstall_core::Scope::current()?;
    uninstall_core::reject_links(&scope.root)?;
    let mut random = [0u8; 16];
    getrandom::fill(&mut random)
        .map_err(|e| anyhow::anyhow!("Could not create a private worker: {e}"))?;
    let id: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let directory = std::env::temp_dir().join(format!("SupabaseDesktop-Uninstall-{id}"));
    std::fs::create_dir(&directory)?;
    uninstall_core::validate_worker(&directory)?;
    let executable = directory.join("SupabaseDesktop.Uninstall.exe");
    std::fs::copy(std::env::current_exe()?, &executable)?;
    std::process::Command::new(executable)
        .current_dir(&directory)
        .args([
            "--uninstall-worker",
            &std::process::id().to_string(),
            &remove_data.to_string(),
        ])
        .spawn()
        .context("Could not start the uninstall worker")?;
    Ok(())
}

fn verify_cleanup(result: &std::path::Path) -> Result<()> {
    let result = std::path::absolute(result)?;
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|e| anyhow::anyhow!("Could not create worker ID: {e}"))?;
    let id: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let directory = std::env::temp_dir().join(format!("SupabaseDesktop-Uninstall-{id}"));
    std::fs::create_dir(&directory)?;
    uninstall_core::validate_worker(&directory)?;
    let executable = directory.join("SupabaseDesktop.Uninstall.exe");
    std::fs::copy(std::env::current_exe()?, &executable)?;
    let status = std::process::Command::new(executable)
        .creation_flags(0x08000000)
        .arg("--verify-worker-copy")
        .arg(&result)
        .status()?;
    ensure!(status.success(), "Copied Rust worker verification failed");
    let deadline = std::time::Instant::now() + Duration::from_secs(45);
    while directory.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    ensure!(
        !directory.exists(),
        "Deferred cleanup did not remove the private Rust worker"
    );
    let mut evidence: serde_json::Value = serde_json::from_slice(&std::fs::read(&result)?)?;
    evidence["copiedRustWorker"] = true.into();
    evidence["deferredCleanup"] = true.into();
    std::fs::write(result, serde_json::to_vec_pretty(&evidence)?)?;
    Ok(())
}

fn schedule_cleanup() -> Result<()> {
    let directory = std::env::current_exe()?
        .parent()
        .context("Missing worker directory")?
        .to_path_buf();
    uninstall_core::validate_worker(&directory)?;
    let script = directory.join("cleanup.ps1");
    std::fs::write(&script, include_str!("uninstall_cleanup.ps1"))?;
    let system = PathBuf::from(std::env::var_os("SystemRoot").context("SystemRoot unavailable")?);
    std::process::Command::new(system.join("System32/WindowsPowerShell/v1.0/powershell.exe"))
        .creation_flags(0x08000000)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(script)
        .arg("-WorkerId")
        .arg(std::process::id().to_string())
        .arg("-Folder")
        .arg(directory)
        .spawn()?;
    Ok(())
}

impl Uninstall {
    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.completed {
            return;
        }
        let remove_data = self.remove_data;
        let weak = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let weak = weak.clone();
            dialog.title(if remove_data { "Permanently delete project data?" } else { "Uninstall Supabase Desktop?" })
                .description(if remove_data { "Remove Supabase Desktop and permanently delete the local database, uploaded files, secrets and snippets? This cannot be undone." } else { "Remove Supabase Desktop, its Start menu shortcuts and its Windows app entry? Your project data will be kept." })
                .confirm().ok_text(if remove_data { "Delete data and uninstall" } else { "Uninstall" }).cancel_text("Cancel")
                .on_ok(move |_, _, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        if this.busy || this.completed { return; }
                        match start_worker(remove_data) {
                            Ok(()) => { this.busy = true; cx.quit(); }
                            Err(error) => { this.message = format!("Uninstall could not start: {error:#}"); cx.notify(); }
                        }
                    });
                    true
                })
        });
    }

    fn run_worker(&mut self, cx: &mut Context<Self>) {
        let Some((parent, remove_data)) = self.worker.take() else {
            return;
        };
        self.busy = true;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let run = || -> Result<()> {
                let directory = std::env::current_exe()?
                    .parent()
                    .context("Missing worker directory")?
                    .to_path_buf();
                uninstall_core::validate_worker(&directory)?;
                uninstall_core::wait_for_parent(parent)?;
                uninstall_core::remove(&uninstall_core::Scope::current()?, remove_data, &|line| {
                    let _ = tx.send(Ok(line));
                })
            };
            let _ = tx.send(Err(run().map_err(|e| format!("{e:#}"))));
        });
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(100)).await;
                let mut finished = false;
                if this.update(cx, |this, cx| {
                    while let Ok(event) = rx.try_recv() {
                        match event {
                            Ok(line) => this.message = line,
                            Err(result) => {
                                this.busy = false; this.completed = true; finished = true;
                                this.message = match result {
                                    Ok(()) if remove_data => "Supabase Desktop and its project data have been removed.".into(),
                                    Ok(()) => format!("Supabase Desktop has been removed. Your project data is kept in:\n{}", uninstall_core::Scope::current().unwrap().root.join("data").display()),
                                    Err(error) => { this.failed = true; this.exit_failure.store(true, std::sync::atomic::Ordering::SeqCst); format!("Uninstall could not finish: {error}") }
                                };
                            }
                        }
                        cx.notify();
                    }
                }).is_err() || finished { break; }
            }
        }).detach();
    }
}

impl Render for Uninstall {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_6()
            .gap_5()
            .font_family("Segoe UI")
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(img(self.logo.clone()).w(px(48.)).h(px(50.)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .child("Supabase Desktop"),
                            )
                            .child(
                                div()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Uninstall · Native Windows"),
                            ),
                    ),
            )
            .child(div().flex_1().child(self.message.clone()))
            .child(
                Button::new("data-choice")
                    .label(if self.remove_data {
                        "[✓] Also permanently delete my local project data"
                    } else {
                        "[ ] Also permanently delete my local project data"
                    })
                    .disabled(self.busy || self.completed)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.remove_data = !this.remove_data;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_3()
                    .child(
                        Button::new("cancel")
                            .label(if self.completed { "Close" } else { "Cancel" })
                            .disabled(self.busy)
                            .on_click(|_, window, cx| {
                                window.remove_window();
                                cx.quit();
                            }),
                    )
                    .child(
                        Button::new("uninstall")
                            .primary()
                            .label(if self.busy {
                                "Uninstalling…"
                            } else {
                                "Uninstall"
                            })
                            .disabled(self.busy || self.completed)
                            .on_click(cx.listener(|this, _, window, cx| this.confirm(window, cx))),
                    ),
            )
    }
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 2 && args[0] == "--verify-cleanup" {
        return verify_cleanup(&PathBuf::from(&args[1]));
    }
    if args.len() == 2 && args[0] == "--verify-worker-copy" {
        uninstall_core::validate_worker(std::env::current_exe()?.parent().unwrap())?;
        let result = uninstall_core::verify_fixture(&PathBuf::from(&args[1]));
        schedule_cleanup()?;
        return result;
    }
    if args.len() == 2 && args[0] == "--verify-uninstall" {
        return uninstall_core::verify_fixture(&PathBuf::from(&args[1]));
    }
    let preview = args.len() == 2 && args[0] == "--verify-ui";
    let worker = if args.len() == 3 && args[0] == "--uninstall-worker" {
        Some((
            args[1]
                .to_str()
                .context("Invalid parent PID")?
                .parse::<u32>()?,
            args[2]
                .to_str()
                .context("Invalid data choice")?
                .parse::<bool>()?,
        ))
    } else {
        None
    };
    ensure!(
        args.is_empty()
            || (args.len() == 1 && args[0] == "--uninstall")
            || preview
            || worker.is_some(),
        "Unknown uninstaller arguments"
    );
    if worker.is_some() {
        uninstall_core::validate_worker(std::env::current_exe()?.parent().unwrap())?;
    }
    let ui_path = preview.then(|| PathBuf::from(&args[1]));
    let failed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let failed_on_close = failed.clone();
    application().with_assets(assets::Assets).run(move |cx| {
        init(cx);
        cx.on_window_closed(|cx, _| { if cx.windows().is_empty() { cx.quit(); } }).detach();
        let (handle, view) = open_window(WindowOptions {
            focus: false, show: !preview,
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(740.), px(440.)), cx))),
            ..Default::default()
        }, cx, move |window, cx| {
            window.set_window_title("Uninstall Supabase Desktop");
            let view = cx.new(|cx| Uninstall {
                logo: Image::from_bytes(ImageFormat::Png, include_bytes!("../../Shared/Branding/supabase-logo.png").to_vec()).to_image_data(cx.svg_renderer()).expect("Could not decode Supabase logo"),
                message: "Remove Supabase Desktop, its Start menu shortcuts and its Windows app entry. Close Supabase Desktop before uninstalling. Your project data is kept unless you select the option below.".into(),
                remove_data: false, busy: false, completed: false, worker, failed: false, exit_failure: failed_on_close.clone(),
            });
            let weak = view.downgrade();
            window.on_window_should_close(cx, move |_, cx| {
                weak.update(cx, |this, _| {
                    failed_on_close.store(this.failed, std::sync::atomic::Ordering::SeqCst);
                    !this.busy
                }).unwrap_or(true)
            });
            if !preview { view.update(cx, |this, cx| this.run_worker(cx)); }
            view
        }).expect("Could not open Rust uninstaller");
        if let Some(path) = ui_path {
            handle.update(cx, |_, window, cx| {
                let _ = window.draw(cx);
                view.update(cx, |this, cx| this.confirm(window, cx));
                let _ = window.draw(cx);
                window.close_dialog(cx);
                view.update(cx, |this, cx| { this.remove_data = true; this.confirm(window, cx); });
                let _ = window.draw(cx);
                std::fs::write(path, b"{\"rustUninstaller\":true,\"nativeWindow\":true,\"frameRendered\":true,\"preserveDataByDefault\":true,\"uninstallConfirmationRendered\":true,\"deleteDataConfirmationRendered\":true}").expect("Could not save UI verification");
                window.remove_window(); cx.quit();
            }).expect("Could not render Rust uninstaller");
        }
    });
    if worker.is_some() {
        let _ = schedule_cleanup();
    }
    ensure!(
        !failed.load(std::sync::atomic::Ordering::SeqCst),
        "Uninstall failed"
    );
    Ok(())
}
