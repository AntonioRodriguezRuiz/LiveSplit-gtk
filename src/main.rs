mod config;
mod ui;

use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use std::time::Duration;

// use api::api::{create, reset, split, start};

use livesplit_core::{Run, Segment, Timer, TimerPhase};
use tracing::info;
use tracing_subscriber;

use adw::prelude::*;
use adw::{Application, ApplicationWindow};
use glib::ControlFlow::Continue;
use gtk4::prelude::*;
use gtk4::{gdk::Display, Box as GtkBox, Builder, Button, CssProvider, Label, Orientation};

use config::Config;
use ui::ui::TimerUI;

fn main() {
    // Set tracing to stdout
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    info!("Staring UnixSplix!!");
    adw::init().expect("Failed to initialize libadwaita");

    let app = Application::builder()
        .application_id("org.LunixRunTools.livesplit-gtk-beta")
        .build();

    let app_state = Rc::new(RefCell::new(LiveSplitGTK::new()));

    app.connect_activate(move |app| {
        app_state.borrow_mut().build_ui(app);
    });
    app.run();
}

#[derive(Clone, Debug)]
pub struct LiveSplitGTK {
    pub timer: Rc<RefCell<Timer>>,
    pub config: Rc<RefCell<Config>>,
}

impl LiveSplitGTK {
    pub fn new() -> Self {
        let config = Config::parse("config.yaml").unwrap_or_default();
        info!(
            "Loading config from {} with result {:?}",
            "config.yaml", config
        );
        let run = config.parse_run_or_default();

        let mut timer = Timer::new(run).expect("Failed to create timer");

        config.configure_timer(&mut timer);

        Self {
            timer: Rc::new(RefCell::new(timer)),
            config: Rc::new(RefCell::new(config)),
        }
    }

    fn load_css() {
        let provider = CssProvider::new();
        provider.load_from_path("data/css/livesplit-gtk.css");

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
        let timer_ui = TimerUI::new(timer_binding.borrow_mut(), config_binding.borrow());
        let ui = timer_ui.build_ui(); // Prevent expiration

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("LiveSplit GTK")
            .default_width(400)
            .default_height(600)
            .content(&ui)
            .build();

        window.present();
    }
}
