#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use gpui_kit::base::Disableable;
mod payload;
mod windows;
use gpui_kit::{
    component::{
        ActiveTheme,
        button::{Button, ButtonVariants},
    },
    *,
};
use std::{path::PathBuf, sync::mpsc, time::Duration};
include!(concat!(env!("OUT_DIR"), "/payload.rs"));

struct Setup {
    message: String,
    busy: bool,
    installed: Option<PathBuf>,
}

impl Setup {
    fn install(&mut self, cx: &mut Context<Self>) {
        self.busy = true;
        self.message = "Checking Windows prerequisites…".into();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| -> anyhow::Result<PathBuf> {
                windows::prerequisites()?;
                let root = PathBuf::from(
                    std::env::var_os("LOCALAPPDATA")
                        .ok_or_else(|| anyhow::anyhow!("LOCALAPPDATA is unavailable"))?,
                )
                .join("SupabaseLocal");
                std::fs::create_dir_all(&root)?;
                let lock = std::fs::OpenOptions::new()
                    .create(true)
                    .truncate(false)
                    .read(true)
                    .write(true)
                    .open(root.join("launcher.lock"))?;
                fs2::FileExt::try_lock_exclusive(&lock).map_err(|_| {
                    anyhow::anyhow!("Close the running Supabase Local app before installing.")
                })?;
                let directory = payload::install(PAYLOAD, &root, &|s| {
                    let _ = tx.send(Ok(s));
                })?;
                windows::register(&directory, &root)?;
                Ok(directory)
            })();
            let _ = tx.send(Err(result.map_err(|e| format!("{e:#}"))));
        });
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(100)).await;
                let mut finished = false;
                if this.update(cx, |this, cx| {
                    while let Ok(event) = rx.try_recv() {
                        match event {
                            Ok(message) => this.message = message,
                            Err(result) => {
                                this.busy = false;
                                finished = true;
                                match result {
                                    Ok(path) => { this.installed = Some(path); this.message = "Installation complete. Open Supabase Local to start your services.".into(); }
                                    Err(error) => this.message = format!("Setup could not finish: {error}"),
                                }
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
            .p_8()
            .gap_6()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(div().text_3xl().child("Supabase Local"))
            .child(div().text_lg().child("Native Windows installer"))
            .child("PostgreSQL · Studio · Auth · REST · Storage")
            .child("Installs for your Windows account. Existing project data is preserved.")
            .child(div().flex_1().child(self.message.clone()))
            .child(
                Button::new("install")
                    .primary()
                    .label(if self.installed.is_some() {
                        "Open Supabase Local"
                    } else if self.busy {
                        "Installing…"
                    } else {
                        "Install Supabase Local"
                    })
                    .disabled(self.busy)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(path) = &this.installed {
                            if let Err(e) = windows::launch(path) {
                                this.message = e.to_string();
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
    if !args.is_empty() {
        anyhow::ensure!(
            args.len() == 2 && args[0] == "--verify-install",
            "Usage: SupabaseLocal.Setup.exe [--verify-install <directory>]"
        );
        let root = PathBuf::from(&args[1]);
        let installed = payload::install(PAYLOAD, &root, &|_| {})?;
        std::fs::write(
            root.join("verification-result.json"),
            serde_json::to_vec(&serde_json::json!({"installed": installed}))?,
        )?;
        return Ok(());
    }
    application().with_assets(assets::Assets).run(|cx| {
        init(cx);
        open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(680.), px(460.)),
                    cx,
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| {
                window.set_window_title("Supabase Local — Setup");
                let view = cx.new(|_| Setup {
                    message: "Ready to install the bundled native runtime.".into(),
                    busy: false,
                    installed: None,
                });
                let weak = view.downgrade();
                window.on_window_should_close(cx, move |_, cx| {
                    weak.update(cx, |this, _| !this.busy).unwrap_or(true)
                });
                view
            },
        )
        .expect("Could not open setup window");
    });
    Ok(())
}
