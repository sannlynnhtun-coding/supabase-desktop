#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app_runtime;
use anyhow::{Context as _, Result};
use app_runtime::{Action, Event, Session, Status};
use gpui_kit::base::Disableable;
use gpui_kit::{
    component::{
        ActiveTheme, WindowExt,
        button::{Button, ButtonVariants},
    },
    *,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tray_icon::{
    TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem},
};

struct Tray {
    icon: TrayIcon,
    show: MenuId,
    studio: MenuId,
    restart: MenuId,
    exit: MenuId,
}
impl Tray {
    fn new() -> Result<Self> {
        let menu = Menu::new();
        let show = MenuItem::new("Show Supabase Desktop", true, None);
        let studio = MenuItem::new("Open Supabase Studio", true, None);
        let restart = MenuItem::new("Restart services…", true, None);
        let exit = MenuItem::new("Exit…", true, None);
        menu.append_items(&[
            &show,
            &studio,
            &PredefinedMenuItem::separator(),
            &restart,
            &exit,
        ])?;
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon(tray_icon::Icon::from_resource(1, Some((32, 32)))?)
            .with_tooltip("Supabase Desktop — starting services")
            .with_menu_on_left_click(false)
            .build()?;
        Ok(Self {
            icon,
            show: show.id().clone(),
            studio: studio.id().clone(),
            restart: restart.id().clone(),
            exit: exit.id().clone(),
        })
    }
}

fn visible(window: &Window, show: bool) {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn ShowWindow(window: isize, command: i32) -> i32;
    }
    if let Ok(handle) = HasWindowHandle::window_handle(window) {
        if let RawWindowHandle::Win32(raw) = handle.as_raw() {
            unsafe {
                ShowWindow(raw.hwnd.get(), if show { 9 } else { 0 });
            }
        }
    }
    if show {
        window.activate_window();
    }
}

struct Desktop {
    logo: Arc<RenderImage>,
    session: Option<Session>,
    tray: Option<Tray>,
    status: Status,
    busy: bool,
    confirmation: app_runtime::Confirmation,
    logs: Vec<String>,
    scroll: ScrollHandle,
    started: Instant,
    data: PathBuf,
}

impl Desktop {
    fn new(window: &mut Window, cx: &mut Context<Self>, preview: bool) -> Self {
        let logo = Image::from_bytes(
            ImageFormat::Png,
            include_bytes!("../../Shared/Branding/supabase-logo.png").to_vec(),
        )
        .to_image_data(cx.svg_renderer())
        .expect("Could not decode Supabase logo");
        let data = app_runtime::default_data().unwrap_or_default();
        let tray_result = Tray::new();
        let mut app = Self {
            logo,
            session: None,
            tray: None,
            status: Status {
                state: "starting".into(),
                message: "Starting your local development environment…".into(),
                ..Default::default()
            },
            busy: !preview,
            confirmation: app_runtime::Confirmation::default(),
            logs: vec![],
            scroll: ScrollHandle::new(),
            started: Instant::now(),
            data,
        };
        match tray_result {
            Ok(tray) => app.tray = Some(tray),
            Err(e) => app.log(format!("Notification icon unavailable: {e}")),
        }
        if !preview {
            match app_runtime::installation_directory() {
                Ok(directory) => {
                    app.session = Some(app_runtime::start_session(directory, app.data.clone()))
                }
                Err(e) => {
                    app.busy = false;
                    app.status.state = "failed".into();
                    app.log(e.to_string());
                }
            }
        } else {
            app.log("Preview: services are not started.".into());
        }
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let result = this.update_in(cx, |this, window, cx| {
                    let mut events = Vec::new();
                    if let Some(session) = &this.session {
                        while let Ok(event) = session.events.try_recv() {
                            events.push(event);
                        }
                    }
                    for event in events {
                        match event {
                            Event::Log(line) => this.log(line),
                            Event::Status(status) => {
                                this.busy = status.studio_url.is_none() && status.state != "failed";
                                if !this.busy {
                                    this.confirmation.finished();
                                }
                                this.status = status;
                                if this.status.state == "failed" {
                                    this.status.available.clear();
                                    this.status.studio_url = None;
                                }
                                if let Some(tray) = &this.tray {
                                    let _ = tray.icon.set_tooltip(Some(format!(
                                        "Supabase Desktop — {}/6 services ready",
                                        this.status.available.len()
                                    )));
                                }
                            }
                            Event::Error(error) => {
                                this.confirmation.finished();
                                this.busy = false;
                                this.status.state = "failed".into();
                                this.status.available.clear();
                                this.status.studio_url = None;
                                this.status.message = error.clone();
                                this.log(format!("ERROR: {error}"));
                            }
                            Event::Stopped => {
                                this.log("All services stopped. Exiting.".into());
                                window.remove_window();
                                cx.quit();
                                return false;
                            }
                        }
                        cx.notify();
                    }
                    while let Ok(event) = MenuEvent::receiver().try_recv() {
                        let command = this.tray.as_ref().and_then(|tray| {
                            if event.id == tray.restart {
                                Some(2)
                            } else if event.id == tray.exit {
                                Some(3)
                            } else if event.id == tray.show {
                                Some(0)
                            } else if event.id == tray.studio {
                                Some(1)
                            } else {
                                None
                            }
                        });
                        match command {
                            Some(0) => visible(window, true),
                            Some(1) => this.open_studio(cx),
                            Some(2) => {
                                visible(window, true);
                                this.request(Action::Restart, window, cx);
                            }
                            Some(3) => {
                                visible(window, true);
                                this.request(Action::Exit, window, cx);
                            }
                            _ => {}
                        }
                    }
                    while let Ok(event) = TrayIconEvent::receiver().try_recv() {
                        if matches!(event, TrayIconEvent::DoubleClick { .. }) {
                            visible(window, true);
                        }
                    }
                    true
                });
                if !matches!(result, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
        app
    }
    fn log(&mut self, line: String) {
        let seconds = self.started.elapsed().as_secs();
        self.logs
            .push(format!("[{:02}:{:02}] {line}", seconds / 60, seconds % 60));
        if self.logs.len() > 500 {
            self.logs.remove(0);
        }
        self.scroll.scroll_to_bottom();
    }
    fn open_studio(&mut self, cx: &mut Context<Self>) {
        if self.status.studio_url.is_some() {
            if let Err(error) = app_runtime::open_url("http://127.0.0.1:54323") {
                self.log(error.to_string());
            }
        } else {
            self.log("Studio is not ready yet. Wait for services to finish starting.".into());
        }
        cx.notify();
    }
    fn request(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        if (self.busy && action == Action::Restart) || !self.confirmation.request(action) {
            return;
        }

        let weak_ok = cx.entity().downgrade();
        let weak_cancel = weak_ok.clone();
        let weak_close = weak_ok.clone();
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let ok = weak_ok.clone();
            let cancel = weak_cancel.clone();
            let close = weak_close.clone();
            dialog
                .title(action.title())
                .description(action.description())
                .confirm()
                .ok_text(action.button())
                .cancel_text("Cancel")
                .on_ok(move |_, window, cx| {
                    let _ = ok.update(cx, |this, cx| {
                        if this.confirmation.confirm(action) != Some(action) {
                            return;
                        }
                        if let Some(session) = &this.session {
                            if session.commands.send(action).is_ok() {
                                this.busy = true;
                                this.status.studio_url = None;
                                this.status.available.clear();
                                this.status.message = match action {
                                    Action::Restart => "Restarting local services…",
                                    Action::Exit => "Stopping local services…",
                                }
                                .into();
                                this.log(format!("{} confirmed.", action.title()));
                            } else {
                                this.confirmation.finished();
                                if action == Action::Exit {
                                    window.remove_window();
                                    cx.quit();
                                    return;
                                }
                                this.log(
                                    "Service controller is unavailable. Check the activity log."
                                        .into(),
                                );
                            }
                        } else if action == Action::Exit {
                            window.remove_window();
                            cx.quit();
                        }
                        cx.notify();
                    });
                    true
                })
                .on_cancel(move |_, _, cx| {
                    let _ = cancel.update(cx, |this, cx| {
                        this.confirmation.cancel();
                        cx.notify();
                    });
                    true
                })
                .on_close(move |_, _, cx| {
                    let _ = close.update(cx, |this, cx| {
                        this.confirmation.cancel();
                        cx.notify();
                    });
                })
        });
        cx.notify();
    }
}

const SERVICES: [(&str, &str, &str); 6] = [
    ("PostgreSQL", "Database", "54322"),
    ("Studio", "Database workspace", "54323"),
    ("Auth", "Authentication", "54328"),
    ("PostgREST", "REST API", "54326"),
    ("Storage", "Files and buckets", "54330"),
    ("postgres-meta", "Database metadata", "54324"),
];

impl Render for Desktop {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ready = self.status.available.len();
        let accent = rgb(0x25996c);
        let services = SERVICES
            .iter()
            .map(|(name, description, port)| {
                let running = self.status.available.iter().any(|s| s == name);
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .py_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(*name))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(*description),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_6()
                            .child(
                                div()
                                    .text_sm()
                                    .font_family("Consolas")
                                    .child(format!("127.0.0.1:{port}")),
                            )
                            .child(
                                div()
                                    .w(px(78.))
                                    .text_sm()
                                    .text_color(if running {
                                        accent.into()
                                    } else {
                                        cx.theme().muted_foreground
                                    })
                                    .child(if running { "Running" } else { "Stopped" }),
                            ),
                    )
            })
            .collect::<Vec<_>>();
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_6()
            .gap_4()
            .font_family("Segoe UI")
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(img(self.logo.clone()).size(px(38.)))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_2xl()
                                            .font_weight(FontWeight::BOLD)
                                            .child("Supabase Desktop"),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("Your local development environment"),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("minimize")
                                    .label("Hide to tray")
                                    .disabled(self.tray.is_none())
                                    .on_click(|_, window, _| visible(window, false)),
                            )
                            .child(
                                Button::new("studio")
                                    .primary()
                                    .label("Open Studio")
                                    .disabled(self.status.studio_url.is_none())
                                    .on_click(cx.listener(|this, _, _, cx| this.open_studio(cx))),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .p_4()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child(
                                if ready == 6 {
                                    "Local stack is ready"
                                } else if self.status.state == "failed" {
                                    "Your attention is needed"
                                } else {
                                    "Preparing your local stack"
                                },
                            ))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(if ready == 6 {
                                        "Studio, database and APIs are available on this computer."
                                            .into()
                                    } else {
                                        self.status.message.clone()
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .text_2xl()
                            .text_color(accent)
                            .child(format!("{ready}/6")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_6()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap_2()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Services"))
                            .child(
                                div()
                                    .id("services")
                                    .overflow_y_scroll()
                                    .flex_1()
                                    .min_h_0()
                                    .children(services),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w(px(330.))
                            .gap_2()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Activity"))
                            .child(
                                div()
                                    .id("activity")
                                    .overflow_y_scroll()
                                    .track_scroll(&self.scroll)
                                    .flex_1()
                                    .min_h_0()
                                    .p_3()
                                    .rounded_md()
                                    .bg(cx.theme().muted)
                                    .font_family("Consolas")
                                    .text_sm()
                                    .children(
                                        self.logs
                                            .iter()
                                            .map(|line| div().mb_2().child(line.clone())),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("Project data: {}", self.data.display())),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pt_3()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("Unofficial native Windows port · No Docker or WSL"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("restart")
                                    .label("Restart services")
                                    .disabled(self.busy || self.session.is_none())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.request(Action::Restart, window, cx)
                                    })),
                            )
                            .child(Button::new("exit").label("Exit").on_click(cx.listener(
                                |this, _, window, cx| this.request(Action::Exit, window, cx),
                            ))),
                    ),
            )
    }
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args
        .first()
        .is_some_and(|s| s == "--uninstall" || s == "--uninstall-worker")
    {
        std::process::Command::new(
            app_runtime::installation_directory()?.join("SupabaseLocal.Uninstall.exe"),
        )
        .args(&args)
        .spawn()
        .context("Could not open the uninstaller")?;
        return Ok(());
    }
    if args.len() == 2 && (args[0] == "--verify-launch" || args[0] == "--verify-restart") {
        return app_runtime::verify_lifecycle(
            &app_runtime::installation_directory()?,
            &PathBuf::from(&args[1]),
            args[0] == "--verify-restart",
        );
    }
    let preview = args.len() == 2 && args[0] == "--verify-ui";
    anyhow::ensure!(args.is_empty() || preview, "Unknown launcher arguments");
    let result_path = preview.then(|| PathBuf::from(&args[1]));
    application().with_assets(assets::Assets).run(move |cx| {
        init(cx);
        cx.on_window_closed(|cx,_|{if cx.windows().is_empty(){cx.quit();}}).detach();
        let (handle, view)=open_window(WindowOptions { focus:false,show:!preview,window_bounds:Some(WindowBounds::Windowed(Bounds::centered(None,size(px(1000.),px(720.)),cx))),..Default::default() },cx,move|window,cx|{
            window.set_window_title("Supabase Desktop");
            let view=cx.new(|cx|Desktop::new(window,cx,preview));
            let weak=view.downgrade();
            window.on_window_should_close(cx,move|window,cx|{let _=weak.update(cx,|this,cx|this.request(Action::Exit,window,cx));false});
            view
        }).expect("Could not open Supabase Desktop");
        if let Some(path)=result_path {
            handle.update(cx,|_,window,cx|{
                let _=window.draw(cx);
                view.update(cx,|this,cx|this.request(Action::Restart,window,cx));
                let _=window.draw(cx);
                assert!(view.read(cx).tray.is_some(), "Notification icon was not created");
                window.close_dialog(cx);
                view.update(cx,|this,cx|{this.confirmation.cancel();this.request(Action::Exit,window,cx);});
                let _=window.draw(cx);
                std::fs::write(path,b"{\"nativeWindow\":true,\"frameRendered\":true,\"restartConfirmationRendered\":true,\"exitConfirmationRendered\":true,\"trayIcon\":true}").expect("Could not save UI verification");
                window.remove_window();cx.quit();
            }).expect("Could not render Supabase Desktop");
        }
    });
    Ok(())
}
