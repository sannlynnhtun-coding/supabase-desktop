#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use gpui_kit::base::Disableable;
mod bundle;
mod payload;
mod windows;
use gpui_kit::{
    component::{
        ActiveTheme,
        button::{Button, ButtonVariants},
    },
    *,
};
use std::{
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

struct Setup {
    logo: std::sync::Arc<RenderImage>,
    message: String,
    busy: bool,
    installed: Option<PathBuf>,
    logs: Vec<String>,
    log_started: Instant,
    log_scroll: ScrollHandle,
}

fn install_for_user(report: &impl Fn(String)) -> anyhow::Result<PathBuf> {
    report("Checking Microsoft C++ runtime…".into());
    windows::prerequisites()?;
    report("Microsoft C++ runtime is ready.".into());
    let root = PathBuf::from(
        std::env::var_os("LOCALAPPDATA")
            .ok_or_else(|| anyhow::anyhow!("LOCALAPPDATA is unavailable"))?,
    )
    .join("SupabaseLocal");
    std::fs::create_dir_all(&root)?;
    report("Checking for another running Supabase Desktop app…".into());
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(root.join("launcher.lock"))?;
    fs2::FileExt::try_lock_exclusive(&lock).map_err(|_| {
        anyhow::anyhow!("Close the running Supabase Desktop app before installing.")
    })?;
    report("Reading and verifying the bundled runtime archive…".into());
    let bytes = bundle::read()?;
    report("Runtime archive checksum verified. Preparing application files…".into());
    let directory = payload::install(&bytes, &root, report)?;
    report("Application files and runtime links are ready.".into());
    report("Creating Start menu shortcuts and Installed apps entry…".into());
    windows::register(&directory, &root)?;
    report("Windows registration completed.".into());
    Ok(directory)
}

impl Setup {
    fn append_log(&mut self, message: String) {
        let seconds = self.log_started.elapsed().as_secs();
        self.logs.push(format!(
            "[{:02}:{:02}] {message}",
            seconds / 60,
            seconds % 60
        ));
        self.log_scroll.scroll_to_bottom();
    }

    fn install(&mut self, cx: &mut Context<Self>) {
        self.busy = true;
        self.message = "Checking Windows prerequisites…".into();
        self.logs.clear();
        self.log_started = Instant::now();
        self.append_log("Starting installation. Existing project data will be preserved.".into());
        cx.notify();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = install_for_user(&|message| {
                let _ = tx.send(Ok(message));
            });
            let _ = tx.send(Err(result.map_err(|e| format!("{e:#}"))));
        });
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(100)).await;
                let mut finished = false;
                if this.update(cx, |this, cx| {
                    while let Ok(event) = rx.try_recv() {
                        match event {
                            Ok(message) => { this.message = message.clone(); this.append_log(message); }
                            Err(result) => {
                                this.busy = false;
                                finished = true;
                                match result {
                                    Ok(path) => { this.installed = Some(path); this.message = "Installation complete. Open Supabase Desktop to start your services.".into(); }
                                    Err(error) => {
                                        this.message = "Setup could not finish. See the installation log below.".into();
                                        this.append_log(format!("ERROR: {error}"));
                                    }
                                }
                                this.append_log(this.message.clone());
                            }
                        }
                        cx.notify();
                    }
                }).is_err() || finished { break; }
            }
        }).detach();
    }
}

impl Render for Setup {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_6()
            .font_family("Segoe UI")
            .gap_4()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(img(self.logo.clone()).w(px(48.)).h(px(50.)).flex_shrink_0())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(22.))
                                    .font_weight(FontWeight::BOLD)
                                    .child("Supabase Desktop"),
                            )
                            .child(
                                div()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Unofficial native Windows port"),
                            ),
                    ),
            )
            .child("PostgreSQL · Studio · Auth · REST · Storage")
            .child("Installs for your Windows account. Existing project data is preserved.")
            .child(div().child(self.message.clone()))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(120.))
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("Installation log"),
                    )
                    .child(
                        div()
                            .id("installation-log")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.log_scroll)
                            .p_3()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().muted)
                            .font_family("Consolas")
                            .text_sm()
                            .children(
                                self.logs
                                    .iter()
                                    .map(|line| div().mb_1().child(line.clone())),
                            ),
                    ),
            )
            .child(
                Button::new("install")
                    .primary()
                    .label(if self.installed.is_some() {
                        "Open Supabase Desktop"
                    } else if self.busy {
                        "Installing…"
                    } else {
                        "Install Supabase Desktop"
                    })
                    .disabled(self.busy)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(path) = &this.installed {
                            if let Err(e) = windows::launch(path) {
                                this.message = e.to_string();
                                this.append_log(this.message.clone());
                                cx.notify();
                            }
                        } else if !this.busy {
                            this.install(cx);
                        }
                    })),
            )
    }
}

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--install" {
        let directory = install_for_user(&|message| println!("{message}"))?;
        println!("Installed: {}", directory.display());
        return Ok(());
    }
    let verify_ui = args.len() == 2 && args[0] == "--verify-ui";
    let ui_path = verify_ui.then(|| PathBuf::from(&args[1]));
    let ui_result = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let ui_result_in_window = ui_result.clone();
    if verify_ui {
        eprintln!("GPUI verification: starting platform");
    }
    if !args.is_empty() && !verify_ui {
        anyhow::ensure!(
            args.len() == 2 && args[0] == "--verify-install",
            "Usage: SupabaseDesktop.Setup.exe [--install | --verify-install <directory> | --verify-ui <result.json>]"
        );
        let root = PathBuf::from(&args[1]);
        let bytes = bundle::read()?;
        let installed = payload::install(&bytes, &root, &|message| println!("{message}"))?;
        std::fs::write(
            root.join("verification-result.json"),
            serde_json::to_vec(&serde_json::json!({"installed": installed}))?,
        )?;
        return Ok(());
    }
    application().with_assets(assets::Assets).run(move |cx| {
        if verify_ui {
            eprintln!("GPUI verification: initializing components");
        }
        init(cx);
        if verify_ui {
            eprintln!("GPUI verification: opening native window");
        }
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let (handle, _) = open_window(
            WindowOptions {
                // The Windows backend retains SW_HIDE for a new focused window.
                // Showing without activation also avoids stealing focus from the user.
                focus: false,
                show: !verify_ui,
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(740.), px(620.)),
                    cx,
                ))),
                ..Default::default()
            },
            cx,
            move |window, cx| {
                if verify_ui {
                    eprintln!("GPUI verification: native window created");
                }
                window.set_window_title("Supabase Desktop — Setup");
                let view = cx.new(|cx| Setup {
                    logo: Image::from_bytes(
                        ImageFormat::Png,
                        include_bytes!("../../Shared/Branding/supabase-logo.png").to_vec(),
                    )
                    .to_image_data(cx.svg_renderer())
                    .expect("Could not decode the original Supabase logo"),
                    message: "Ready to install the bundled native runtime.".into(),
                    busy: false,
                    installed: None,
                    logs: vec!["Ready. Select Install Supabase Desktop to begin.".into()],
                    log_started: Instant::now(),
                    log_scroll: ScrollHandle::new(),
                });
                let weak = view.downgrade();
                window.on_window_should_close(cx, move |_, cx| {
                    weak.update(cx, |this, _| !this.busy).unwrap_or(true)
                });
                view
            },
        )
        .expect("Could not open setup window");
        if let Some(path) = ui_path {
            handle
                .update(cx, |_, window, cx| {
                    // Hidden windows have no display-driven frames; draw the same
                    // mounted view explicitly against the native renderer for this check.
                    let _ = window.draw(cx);

                    eprintln!("GPUI verification: frame rendered");
                    let verified =
                        std::fs::write(path, b"{\"nativeWindow\":true,\"frameRendered\":true}")
                            .is_ok();
                    ui_result_in_window.store(verified, std::sync::atomic::Ordering::SeqCst);
                    window.remove_window();
                    cx.quit();
                })
                .expect("Could not render setup window");
        }
    });
    anyhow::ensure!(
        !verify_ui || ui_result.load(std::sync::atomic::Ordering::SeqCst),
        "GPUI window verification did not complete"
    );
    Ok(())
}
