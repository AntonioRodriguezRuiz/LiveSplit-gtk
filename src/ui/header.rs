use std::sync::{Arc, RwLock};

use adw::prelude::*;
use adw::{self, AboutDialog, AlertDialog};
use gtk4::{
    gio, Align, Box as GtkBox, FileChooserDialog, FileFilter, Label, ListBox, MenuButton,
    Orientation::Vertical,
};
use livesplit_core::Timer;

use crate::config::Config;

/// TuxSplitHeader
/// A top bar that renders the application title and a hamburger menu.
pub struct TuxSplitHeader {
    header: adw::HeaderBar,
    menu: TuxSplitMenu,
}

impl TuxSplitHeader {
    pub fn new(
        parent: &adw::ApplicationWindow,
        timer: Arc<RwLock<Timer>>,
        config: Arc<RwLock<Config>>,
    ) -> Self {
        let header = adw::HeaderBar::builder()
            .show_end_title_buttons(true)
            .build();

        let menu = TuxSplitMenu::new(parent, timer, config);
        header.pack_start(menu.button());

        Self { header, menu }
    }

    pub fn header(&self) -> &adw::HeaderBar {
        &self.header
    }
}

pub struct TuxSplitMenu {
    button: MenuButton,
}

impl TuxSplitMenu {
    pub fn new(
        parent: &adw::ApplicationWindow,
        timer: Arc<RwLock<Timer>>,
        config: Arc<RwLock<Config>>,
    ) -> Self {
        let button = MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .build();

        let menu = gio::Menu::new();

        let splits_section = gio::Menu::new();
        splits_section.append(Some("Load Splits"), Some("app.load-splits"));
        splits_section.append(Some("Save Splits"), Some("app.save-splits"));

        let settings_section = gio::Menu::new();
        settings_section.append(Some("Settings"), Some("app.settings"));
        settings_section.append(Some("Keybindings"), Some("app.keybindings"));

        let about_section = gio::Menu::new();
        about_section.append(Some("About"), Some("app.about"));

        menu.append_section(None, &splits_section);
        menu.append_section(None, &settings_section);
        menu.append_section(None, &about_section);
        button.set_menu_model(Some(&menu));

        // Actions
        let group = gio::SimpleActionGroup::new();
        group.add_action(&Self::get_load_action(
            parent,
            timer.clone(),
            config.clone(),
        ));
        group.add_action(&Self::get_save_action(timer.clone(), config.clone()));
        group.add_action(&Self::get_settings_action(parent));
        group.add_action(&Self::get_keybinds_action(parent));
        group.add_action(&Self::get_about_action(parent));
        button.insert_action_group("app", Some(&group));

        Self { button }
    }

    pub fn button(&self) -> &MenuButton {
        &self.button
    }

    fn get_save_action(
        timer: Arc<RwLock<Timer>>,
        config: Arc<RwLock<Config>>,
    ) -> gio::SimpleAction {
        let action = gio::SimpleAction::new("save-splits", None);
        action.connect_activate(move |_, _| {
            let t = timer.read().unwrap();
            let c = config.read().unwrap();
            c.save_splits(&t);
        });
        action
    }

    fn get_load_action(
        parent: &adw::ApplicationWindow,
        timer: Arc<RwLock<Timer>>,
        config: Arc<RwLock<Config>>,
    ) -> gio::SimpleAction {
        let parent_binding = parent.clone();
        let action = gio::SimpleAction::new("load-splits", None);
        action.connect_activate(move |_, _| {
            let file_chooser = FileChooserDialog::new(
                Some("Load Splits"),
                Some(&parent_binding),
                gtk4::FileChooserAction::Open,
                &[
                    ("Open", gtk4::ResponseType::Ok),
                    ("Cancel", gtk4::ResponseType::Cancel),
                ],
            );

            let lss_filter = FileFilter::new();
            let all_filter = FileFilter::new();
            lss_filter.set_name(Some("LiveSplit Splits (*.lss)"));
            all_filter.set_name(Some("All Files"));
            lss_filter.add_pattern("*.lss");
            all_filter.add_pattern("*");
            file_chooser.add_filter(&lss_filter);
            file_chooser.add_filter(&all_filter);

            let t_binding = timer.clone();
            let c_binding = config.clone();
            file_chooser.connect_response(move |dialog, response| {
                if response == gtk4::ResponseType::Ok {
                    if let Some(file) = dialog.file() {
                        if let Some(path) = file.path() {
                            let mut c = c_binding.write().unwrap();
                            c.set_splits_path(path);
                            if let Some(run) = c.parse_run() {
                                let mut t = t_binding.write().unwrap();
                                let _ = t.set_run(run);
                                c.configure_timer(&mut t);
                            }
                        }
                    }
                }
                dialog.destroy();
            });

            file_chooser.set_modal(true);
            file_chooser.present();
        });
        action
    }

    fn get_keybinds_action(parent: &adw::ApplicationWindow) -> gio::SimpleAction {
        let parent_for_keybinds = parent.clone();
        let action = gio::SimpleAction::new("keybindings", None);
        action.connect_activate(move |_, _| {
            let dialog = AlertDialog::builder()
                .heading("Keybindings")
                .body("Current keybinds are not modifiable yet.")
                .default_response("ok")
                .build();

            let keybinds_list = ListBox::new();
            keybinds_list.add_css_class("boxed-list");
            let keybinds = vec![
                ("Start / Split", "Numpad 1"),
                ("Skip Split", "Numpad 2"),
                ("Reset", "Numpad 3"),
                ("Previous Comparison", "Numpad 4"),
                ("Pause", "Numpad 5"),
                ("Next Comparison", "Numpad 6"),
                ("Undo", "Numpad 8"),
            ];
            for (action, key) in keybinds {
                let key_label = Label::new(Some(key));
                let row = adw::ActionRow::builder().title(action).build();
                row.add_suffix(&key_label);
                keybinds_list.append(&row);
            }

            dialog.set_extra_child(Some(&keybinds_list));

            dialog.add_response("ok", "Okay");
            dialog.present(Some(&parent_for_keybinds));
        });
        action
    }

    fn get_settings_action(parent: &adw::ApplicationWindow) -> gio::SimpleAction {
        let parent_for_settings = parent.clone();
        let action = gio::SimpleAction::new("settings", None);
        action.connect_activate(move |_, _| {
            let dialog = AlertDialog::builder()
                .heading("Settings")
                .body("This feature isn't available yet. Stay tuned!")
                .default_response("ok")
                .build();
            dialog.add_response("ok", "Okay");
            dialog.present(Some(&parent_for_settings));
        });
        action
    }

    fn get_about_action(parent: &adw::ApplicationWindow) -> gio::SimpleAction {
        let parent_for_about = parent.clone();
        let action = gio::SimpleAction::new("about", None);
        action.connect_activate(move |_, _| {
            let about_dialog = AboutDialog::builder()
                .application_name("TuxSplit")
                .version("0.0.1")
                .comments("A GTK-based LiveSplit timer application.")
                .license_type(gtk4::License::MitX11)
                .website("https://github.com/AntonioRodriguezRuiz/tuxsplit")
                .build();
            about_dialog.present(Some(&parent_for_about));
        });
        action
    }
}

fn simple_title_header(title: &str) -> adw::HeaderBar {
    let header = adw::HeaderBar::builder()
        .title_widget(
            &GtkBox::builder()
                .orientation(Vertical)
                .halign(Align::Center)
                .build(),
        )
        .show_end_title_buttons(true)
        .build();

    if let Some(label) = header
        .title_widget()
        .and_then(|w| w.downcast::<GtkBox>().ok())
    {
        let lbl = Label::new(Some(title));
        label.append(&lbl);
    }

    header
}
