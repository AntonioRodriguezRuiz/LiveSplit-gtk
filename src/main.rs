mod config;
mod context;
mod formatters;
mod ui;
mod utils;

use std::path::Path;

use tracing::info;

use crate::context::{TuxSplitContext, build_ui, shutdown};
use adw::Application;
use adw::prelude::*;
use gtk4::{
    CssProvider,
    gdk::Display,
    gio::{self},
};

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
        .application_id("io.github.tunixr.tuxsplit")
        .build();

    let app_ctx = TuxSplitContext::new_initialized();

    {
        let ctx_binding = app_ctx.clone();
        app.connect_activate(move |app| {
            load_styles();
            build_ui(&ctx_binding, app);
        });
    }
    {
        let ctx_binding = app_ctx.clone();
        app.connect_shutdown(move |_| {
            shutdown(&ctx_binding);
        });
    }
    app.run();
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
