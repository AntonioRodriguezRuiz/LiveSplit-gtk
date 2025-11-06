mod config;
mod formatters;
mod ui;

use std::sync::{Arc, RwLock};

use livesplit_core::{auto_splitting::Runtime, HotkeySystem, Timer};
use tracing::info;

use adw::prelude::*;
use adw::{Application, ApplicationWindow, ToolbarView};
use gtk4::{gdk::Display, CssProvider};

use config::Config;
use ui::timer::TuxSplitTimer;
use ui::TuxSplitHeader;

fn main() {
    std::env::set_var("GDK_BACKEND", "x11"); // Livesplit-core does not support Wayland global shortcut portal yet

    // Set tracing to stdout
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    info!("Staring UnixSplix!!");
    adw::init().expect("Failed to initialize libadwaita");

    let app = Application::builder()
        .application_id("org.LunixRunTools.tuxsplit-beta")
        .build();

    let app_state = Arc::new(RwLock::new(TuxSplit::new()));

    app.connect_activate(move |app| {
        app_state.write().unwrap().build_ui(app);
    });
    app.run();
}

pub struct TuxSplit {
    pub timer: Arc<RwLock<Timer>>,
    pub runtime: Runtime,
    pub config: Arc<RwLock<Config>>,
    pub hotkey_system: Arc<RwLock<HotkeySystem>>,
}

impl Default for TuxSplit {
    fn default() -> Self {
        Self::new()
    }
}

impl TuxSplit {
    #[must_use]
    /// # Panics
    ///
    /// Will panic if the timer or hotkey system cannot be created.
    pub fn new() -> Self {
        let config = Config::parse("config.yaml").unwrap_or_default();
        let run = config.parse_run_or_default();

        let timer = Timer::new(run).expect("Failed to create timer");

        let stimer = timer.into_shared();

        let runtime = Runtime::new(stimer.clone());

        config.configure_timer(&mut stimer.write().unwrap());
        config.maybe_load_auto_splitter(&runtime);

        let Some(hotkey_system) = config.create_hotkey_system(stimer.clone()) else {
            panic!("Could not load HotkeySystem")
        };

        Self {
            timer: stimer,
            runtime,
            config: Arc::new(RwLock::new(config)),
            hotkey_system: Arc::new(RwLock::new(hotkey_system)),
        }
    }

    fn load_css() {
        let provider = CssProvider::new();
        provider.load_from_path("data/css/tuxsplit.css");

        let display = Display::default().expect("Could not connect to a display");
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    fn build_ui(&mut self, app: &Application) {
        Self::load_css();

        let window: ApplicationWindow = ApplicationWindow::builder()
            .application(app)
            .title("TuxSplit")
            .build();

        let toolbar_view = ToolbarView::new();
        let header = TuxSplitHeader::new(&window, self.timer.clone(), self.config.clone());
        toolbar_view.add_top_bar(header.header());

        let mut timer_widget = TuxSplitTimer::new(self.timer.clone(), self.config.clone());
        timer_widget.start_refresh_loop();
        toolbar_view.set_content(Some(timer_widget.clamped()));

        window.set_content(Some(&toolbar_view));
        window.present();
    }
}
