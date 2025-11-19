use adw::{
    ComboRow, ExpanderRow, PreferencesDialog, PreferencesGroup, PreferencesPage, SpinRow,
    SwitchRow, prelude::*,
};
use gtk4::{self as gtk, StringList};
use livesplit_core::TimingMethod;

#[derive(Clone, Copy)]
enum FormatTarget {
    Timer,
    Split,
    Segment,
    Comparison,
}

pub struct TimerPreferencesDialog {
    dialog: PreferencesDialog,
}

impl TimerPreferencesDialog {
    pub fn new() -> Self {
        let dialog = PreferencesDialog::new();
        dialog.set_height_request(500);
        dialog.set_title("Timer Preferences");

        let this = Self { dialog };

        let general = this.build_general_page();
        let style = this.build_style_page();
        let format = this.build_format_page();

        this.dialog.add(&general);
        this.dialog.add(&style);
        this.dialog.add(&format);

        this
    }

    pub fn dialog(&self) -> &PreferencesDialog {
        &self.dialog
    }

    pub fn present(&self, parent: &impl IsA<gtk::Widget>) {
        self.dialog.present(Some(parent));
    }

    // ------------- Pages -------------

    fn build_general_page(&self) -> PreferencesPage {
        let page = PreferencesPage::builder()
            .title("General")
            .icon_name("gears-symbolic")
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
            .icon_name("large-brush-symbolic")
            .build();

        let segments_group = PreferencesGroup::builder().title("Segments").build();

        let (max_segments, follow_from) = {
            let ctx = crate::context::TuxSplitContext::get_instance();
            let c = ctx.config();
            let max_segments = c.style.max_segments_displayed.unwrap_or(10) as f64;
            let follow_from = c.style.segments_scroll_follow_from.unwrap_or(8) as f64;
            (max_segments, follow_from)
        };

        // Scroll follow from
        let follow_from_row = SpinRow::with_range(0.0, max_segments, 1.0);
        follow_from_row.set_title("Scroll follow from");
        follow_from_row.set_value(follow_from);
        follow_from_row.connect_value_notify(move |r| {
            let value = r.value().round().clamp(0.0, 1000.0) as usize;
            if let Ok(mut cfg) = crate::context::TuxSplitContext::get_instance().config_mut() {
                cfg.style.segments_scroll_follow_from = Some(value);
            }
        });

        // Max segments displayed
        let max_segments_row = SpinRow::with_range(1.0, 1000.0, 1.0);
        max_segments_row.set_title("Max segments displayed");
        max_segments_row.set_value(max_segments);
        let follow_from_row_binding = follow_from_row.clone();
        max_segments_row.connect_value_notify(move |r| {
            let value = r.value().round().clamp(1.0, 1000.0) as usize;
            if let Ok(mut cfg) = crate::context::TuxSplitContext::get_instance().config_mut() {
                cfg.style.max_segments_displayed = Some(value);
            }

            // Adjust follow_from if necessary
            follow_from_row_binding.set_range(0.0, value as f64);
            if value < follow_from_row_binding.value() as usize {
                follow_from_row_binding.set_value(value as f64);
            }
        });

        // Show Icons
        let show_icons_row = SwitchRow::builder()
            .title("Show Segment Icons")
            .subtitle("Toggle the display of icons next to segment names")
            .build();
        let initial_show_icons = {
            let ctx = crate::context::TuxSplitContext::get_instance();
            let c = ctx.config();
            c.style.show_icons.unwrap_or(true)
        };
        show_icons_row.set_active(initial_show_icons);
        show_icons_row.connect_active_notify(move |r| {
            let ctx = crate::context::TuxSplitContext::get_instance();
            let active = r.is_active();
            if let Ok(mut cfg) = ctx.config_mut() {
                cfg.style.show_icons = Some(active);
                drop(cfg);
                ctx.emit_by_name::<()>("run-changed", &[]);
            }
        });

        segments_group.add(&max_segments_row);
        segments_group.add(&follow_from_row);
        segments_group.add(&show_icons_row);

        page.add(&segments_group);
        page
    }

    fn build_format_page(&self) -> PreferencesPage {
        let page = PreferencesPage::builder()
            .title("Format")
            .icon_name("headings-symbolic")
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

        let comparison_expander = self.build_format_expander(
            "Comparison Times Format",
            "Controls formatting of the comparison info section.",
            FormatTarget::Comparison,
        );
        formats_group.add(&comparison_expander);

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
            let ctx = crate::context::TuxSplitContext::get_instance();
            let c = ctx.config();
            match c.general.timing_method {
                Some(TimingMethod::GameTime) => 1,
                _ => 0, // default Real Time
            }
        };
        row.set_selected(initial_selected);

        row.connect_selected_notify(move |r| {
            let selected = r.selected();
            let method = if selected == 1 {
                TimingMethod::GameTime
            } else {
                TimingMethod::RealTime
            };

            if let Ok(mut cfg) = crate::context::TuxSplitContext::get_instance().config_mut() {
                cfg.general.timing_method = Some(method);
            }

            if let Ok(mut t) = crate::context::TuxSplitContext::get_instance()
                .timer()
                .try_write()
            {
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
            let ctx = crate::context::TuxSplitContext::get_instance();
            let cfg = ctx.config();
            let tf = match target {
                FormatTarget::Timer => &cfg.format.timer,
                FormatTarget::Split => &cfg.format.split,
                FormatTarget::Segment => &cfg.format.segment,
                FormatTarget::Comparison => &cfg.format.comparison,
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

        mode_row.connect_selected_notify(move |r| {
            let idx = r.selected();
            if let Ok(mut cfg) = crate::context::TuxSplitContext::get_instance().config_mut() {
                let tf = match target {
                    FormatTarget::Timer => &mut cfg.format.timer,
                    FormatTarget::Split => &mut cfg.format.split,
                    FormatTarget::Segment => &mut cfg.format.segment,
                    FormatTarget::Comparison => &mut cfg.format.comparison,
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
            }
        });

        decimals_row.connect_value_notify(move |row| {
            let val = row.value().round().clamp(1.0, 3.0) as u8;
            if let Ok(mut cfg) = crate::context::TuxSplitContext::get_instance().config_mut() {
                let tf = match target {
                    FormatTarget::Timer => &mut cfg.format.timer,
                    FormatTarget::Split => &mut cfg.format.split,
                    FormatTarget::Segment => &mut cfg.format.segment,
                    FormatTarget::Comparison => &mut cfg.format.comparison,
                };
                tf.set_decimal_places(val);
            }
        });

        expander.add_row(&mode_row);
        expander.add_row(&decimals_row);

        expander
    }
}
