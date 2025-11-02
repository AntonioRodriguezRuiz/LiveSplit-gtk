mod config;
mod formatters;
mod ui;
mod utils;

use std::sync::{Arc, RwLock};

use livesplit_core::{HotkeySystem, Timer};
use tracing::info;
use tracing_subscriber;

use adw::prelude::*;
use adw::Application;
use gtk4::{gdk::Display, CssProvider};

use config::Config;
use ui::timer::TimerUI;

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

#[derive(Clone)]
pub struct TuxSplit {
    pub timer: Arc<RwLock<Timer>>,
    pub config: Arc<RwLock<Config>>,
    pub hotkey_system: Arc<RwLock<HotkeySystem>>,
}

impl TuxSplit {
    pub fn new() -> Self {
        let config = Config::parse("config.yaml").unwrap_or_default();
        let run = config.parse_run_or_default();

        let timer = Timer::new(run).expect("Failed to create timer");

        let stimer = timer.into_shared();

        config.configure_timer(&mut stimer.write().unwrap());

        let Some(hotkey_system) = config.create_hotkey_system(stimer.clone()) else {
            panic!("Could not load HotkeySystem")
        };

        Self {
            timer: stimer,
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

        // TODO: Change this to use main ui, from then render timer ui when loading a file
        // To ensure changes in config and timer translate
        let timer_binding = self.timer.clone();
        let config_binding = self.config.clone();
        let timer_ui = TimerUI::new(timer_binding, config_binding);

        let window = timer_ui.build_ui(app);

        window.present();
        // timer_ui.spawn_debug_ui();
    }
}
