mod config;
mod formatters;
mod ui;
mod utils;

use std::{
    env,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, RwLock},
};

use livesplit_core::{HotkeySystem, Timer, auto_splitting::Runtime};
use tracing::info;

use adw::prelude::*;
use adw::{Application, ApplicationWindow, ToolbarView};
use gtk4::{
    CssProvider,
    gdk::Display,
    gio::{self},
};

use config::Config;
use ui::TuxSplitHeader;
use ui::timer::TuxSplitTimer;

const RESOURCE_ICONS: &str = "/com/tunixr/tuxsplit/icons";
const RESOURCE_CSS: &str = "/com/tunixr/tuxsplit/css/tuxsplit.css";

fn main() {
    unsafe {
        std::env::set_var("GDK_BACKEND", "x11"); // Livesplit-core does not support Wayland global shortcut portal yet
    }

    // Set tracing to stdout
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    register_gresource();
    info!("Starting TuxSplit");
    adw::init().expect("Failed to initialize libadwaita");

    let app = Application::builder()
        .application_id("com.tunixr.tuxsplit")
        .build();

    let app_state = Arc::new(RwLock::new(TuxSplit::new()));

    {
        let state_binding = app_state.clone();
        app.connect_activate(move |app| {
            state_binding.write().unwrap().build_ui(app);
        });
    }
    {
        let state_binding = app_state.clone();
        app.connect_shutdown(move |_| {
            state_binding.read().unwrap().shutdown();
        });
    }
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
        let config = load_config();
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

    fn load_styles() {
        let display = Display::default().expect("Could not connect to a display");
        let css_provider = CssProvider::new();
        css_provider.load_from_resource(RESOURCE_CSS);

        let display_theme = gtk4::IconTheme::for_display(&display);
        display_theme.add_resource_path(RESOURCE_ICONS);

        gtk4::style_context_add_provider_for_display(
            &display,
            &css_provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    fn build_ui(&mut self, app: &Application) {
        Self::load_styles();

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

    fn shutdown(&self) {
        info!("Shutting down TuxSplit");
        let cfg = self.config.read().unwrap();
        // let timer = self.timer.read().unwrap();
        cfg.save(get_config_path().join("config.yaml"))
            .expect("Failed to save config on shutdown");
    }
}

fn register_gresource() {
    let path = Path::new("/app/share/tuxsplit.gresource");
    if path.exists() {
        let res = gio::Resource::load(path).expect("Failed to load resource");
        info!("Registered GResource from {}", path.display());
        gio::resources_register(&res);
        return;
    }
    let usr_path = Path::new("/usr/share/tuxsplit/tuxsplit.gresource");
    if usr_path.exists() {
        let res = gio::Resource::load(usr_path).expect("Failed to load resource");
        info!("Registered GResource from {}", usr_path.display());
        gio::resources_register(&res);
        return;
    }
    panic!("Could not load resources");
}

fn load_config() -> Config {
    let user_cfg = get_config_path().join("config.yaml");
    if user_cfg.is_file()
        && let Some(cfg) = Config::parse(&user_cfg)
    {
        info!("Loaded user config {}", user_cfg.display());
        return cfg;
    }

    Config::default()
}

fn get_config_path() -> PathBuf {
    if let Ok(path_str) = env::var("TUXSPLIT_DATADIR") {
        PathBuf::from(&path_str)
    } else if let Ok(path_str) = env::var("XDG_CONFIG_HOME") {
        path_str.into()
    } else if let Ok(home) = env::var("HOME") {
        let path = PathBuf::from(home).join(".config").join("tuxsplit");
        if !path.is_dir() {
            std::fs::create_dir_all(&path).expect("Failed to create config directory");
        }
        path
    } else {
        PathBuf::from("/tmp")
    }
}
