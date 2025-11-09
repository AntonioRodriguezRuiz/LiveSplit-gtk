use std::sync::{Arc, RwLock};

use adw::{
    ComboRow, ExpanderRow, PreferencesDialog, PreferencesGroup, PreferencesPage, SpinRow,
    prelude::*,
};
use gtk4::{self as gtk, StringList};
use livesplit_core::{Timer, TimingMethod};

use crate::config::Config;

#[derive(Clone, Copy)]
enum FormatTarget {
    Timer,
    Split,
    Segment,
}

pub struct TimerPreferencesDialog {
    dialog: PreferencesDialog,
    timer: Arc<RwLock<Timer>>,
    config: Arc<RwLock<Config>>,
}

impl TimerPreferencesDialog {
    pub fn new(timer: Arc<RwLock<Timer>>, config: Arc<RwLock<Config>>) -> Self {
        let dialog = PreferencesDialog::new();
        dialog.set_height_request(500);
        dialog.set_title("Timer Preferences");

        let this = Self {
            dialog,
            timer,
            config,
        };

        let general = this.build_general_page();
        let style = this.build_style_page();
        let format = this.build_format_page();

        this.dialog.add(&general);
        this.dialog.add(&style);
        this.dialog.add(&format);

        this
    }

    pub fn present(&self, parent: &impl IsA<gtk::Widget>) {
        self.dialog.present(Some(parent));
    }

    // ------------- Pages -------------

    fn build_general_page(&self) -> PreferencesPage {
        let page = PreferencesPage::builder()
            .title("General")
            .icon_name("cogged-wheel-symbolic")
            .build();

        let timing_group = PreferencesGroup::builder().title("Timing").build();

        let timing_row = self.build_timing_method_row();
        timing_group.add(&timing_row);

        page.add(&timing_group);
        page
    }

    fn build_style_page(&self) -> PreferencesPage {
        let page = PreferencesPage::builder()
            .title("Style")
            .icon_name("paintbrush-symbolic")
            .build();

        let segments_group = PreferencesGroup::builder().title("Segments").build();

        let (max_segments, follow_from) = {
            let c = self.config.read().unwrap();
            let max_segments = c.style.max_segments_displayed.unwrap_or(10) as f64;
            let follow_from = c.style.segments_scroll_follow_from.unwrap_or(8) as f64;
            (max_segments, follow_from)
        };

        // Scroll follow from
        let follow_from_row = SpinRow::with_range(0.0, max_segments, 1.0);
        follow_from_row.set_title("Scroll follow from");
        follow_from_row.set_value(follow_from);
        let config = Arc::clone(&self.config);
        follow_from_row.connect_value_notify(move |r| {
            let value = r.value().round().clamp(0.0, 1000.0) as usize;
            let mut cfg = config.write().unwrap();
            cfg.style.segments_scroll_follow_from = Some(value);
            drop(cfg);
        });

        // Max segments displayed
        let max_segments_row = SpinRow::with_range(1.0, 1000.0, 1.0);
        max_segments_row.set_title("Max segments displayed");
        max_segments_row.set_value(max_segments);
        let config = Arc::clone(&self.config);
        let follow_from_row_binding = follow_from_row.clone();
        max_segments_row.connect_value_notify(move |r| {
            let value = r.value().round().clamp(1.0, 1000.0) as usize;
            let mut cfg = config.write().unwrap();
            cfg.style.max_segments_displayed = Some(value);
            drop(cfg); // We need to drop the lock becuase it might be required by follow_from adjustment when the value is modified

            // Adjust follow_from if necessary
            follow_from_row_binding.set_range(0.0, value as f64);
            if value < follow_from_row_binding.value() as usize {
                follow_from_row_binding.set_value(value as f64);
            }
        });

        segments_group.add(&max_segments_row);
        segments_group.add(&follow_from_row);

        page.add(&segments_group);
        page
    }

    fn build_format_page(&self) -> PreferencesPage {
        let page = PreferencesPage::builder()
            .title("Format")
            .icon_name("preferences-system-time")
            .build();

        let formats_group = PreferencesGroup::builder().title("Time Formats").build();

        let timer_expander = self.build_format_expander(
            "Timer Format",
            "Controls the formatting of the running timer.",
            FormatTarget::Timer,
        );
        formats_group.add(&timer_expander);

        let split_expander = self.build_format_expander(
            "Split Times Format",
            "Controls formatting of the delta (split) times.",
            FormatTarget::Split,
        );
        formats_group.add(&split_expander);

        let segment_expander = self.build_format_expander(
            "Segment Times Format",
            "Controls formatting of individual segment durations.",
            FormatTarget::Segment,
        );
        formats_group.add(&segment_expander);

        page.add(&formats_group);
        page
    }

    // ------------- Rows -------------

    fn build_timing_method_row(&self) -> ComboRow {
        let model = StringList::new(&["Real Time", "Game Time"]);
        let row = ComboRow::builder()
            .title("Timing Method")
            .subtitle("Choose which timing method to display and operate with")
            .build();
        row.set_model(Some(&model));

        let initial_selected = {
            let c = self.config.read().unwrap();
            match c.general.timing_method {
                Some(TimingMethod::GameTime) => 1,
                _ => 0, // default Real Time
            }
        };
        row.set_selected(initial_selected);

        let timer = Arc::clone(&self.timer);
        let config = Arc::clone(&self.config);

        row.connect_selected_notify(move |r| {
            let selected = r.selected();
            let method = if selected == 1 {
                TimingMethod::GameTime
            } else {
                TimingMethod::RealTime
            };

            {
                let mut cfg = config.write().unwrap();
                cfg.general.timing_method = Some(method);
            }

            {
                let mut t = timer.write().unwrap();
                t.set_current_timing_method(method);
            }
        });

        row
    }

    fn build_format_expander(
        &self,
        title: &str,
        subtitle: &str,
        target: FormatTarget,
    ) -> ExpanderRow {
        let (initial_mode_index, initial_decimals) = {
            let cfg = self.config.read().unwrap();
            let tf = match target {
                FormatTarget::Timer => &cfg.format.timer,
                FormatTarget::Split => &cfg.format.split,
                FormatTarget::Segment => &cfg.format.segment,
            };
            let mode = if tf.show_decimals {
                u32::from(tf.dynamic)
            } else {
                2
            };
            (mode, tf.decimal_places)
        };

        let expander = ExpanderRow::builder()
            .title(title)
            .subtitle(subtitle)
            .build();

        let mode_model = StringList::new(&["Show decimals", "Smart decimals", "No decimals"]);
        let mode_row = ComboRow::builder()
            .title("Mode")
            .subtitle("Select decimal visibility strategy")
            .build();
        mode_row.set_model(Some(&mode_model));
        mode_row.set_selected(initial_mode_index);

        let decimals_row = SpinRow::with_range(1.0, 3.0, 1.0);
        decimals_row.set_title("Decimal places");
        decimals_row.set_value(f64::from(initial_decimals));

        let config_for_mode = Arc::clone(&self.config);
        mode_row.connect_selected_notify(move |r| {
            let idx = r.selected();
            let mut cfg = config_for_mode.write().unwrap();
            let tf = match target {
                FormatTarget::Timer => &mut cfg.format.timer,
                FormatTarget::Split => &mut cfg.format.split,
                FormatTarget::Segment => &mut cfg.format.segment,
            };
            match idx {
                0 => {
                    // Show decimals
                    tf.dynamic = false;
                    tf.show_decimals = true;
                }
                1 => {
                    // Smart decimals
                    tf.dynamic = true;
                    tf.show_decimals = true;
                }
                2 => {
                    // No decimals
                    tf.dynamic = false;
                    tf.show_decimals = false;
                }
                _ => {}
            }
            tf.set_decimal_places(tf.decimal_places);
        });

        let config_for_decimals = Arc::clone(&self.config);
        decimals_row.connect_value_notify(move |row| {
            let val = row.value().round().clamp(1.0, 3.0) as u8;
            let mut cfg = config_for_decimals.write().unwrap();
            let tf = match target {
                FormatTarget::Timer => &mut cfg.format.timer,
                FormatTarget::Split => &mut cfg.format.split,
                FormatTarget::Segment => &mut cfg.format.segment,
            };
            tf.set_decimal_places(val);
        });

        expander.add_row(&mode_row);
        expander.add_row(&decimals_row);

        expander
    }
}
