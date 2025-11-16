use livesplit_core::{Run, Timer, TimingMethod};
use std::rc::Rc;
use std::sync::{Arc, RwLock};

use gtk4::{Box as GtkBox, ColumnView, ColumnViewColumn, ScrolledWindow, prelude::*};

use crate::formatters::time::parse_hms;
use crate::ui::editor::context::SegmentMoveDirection;
use crate::ui::editor::row::SegmentRow;
use crate::ui::editor::{EditorContext, SegmentsModel};

pub struct SegmentsEditor {
    container: GtkBox,
    table: ColumnView,
    model: gtk4::SingleSelection,
    timer: Arc<RwLock<Timer>>,
    run_snapshot: Run, // Snapshot of the timer at the moment of opening the editor
    timing_method: Arc<RwLock<TimingMethod>>,
    context: EditorContext,
    segments_model: SegmentsModel,
}

impl SegmentsEditor {
    pub fn new(context: EditorContext) -> Rc<Self> {
        let timer = context.timer();
        let run_snapshot = timer.read().unwrap().run().clone();
        let timing_method = Arc::new(RwLock::new(TimingMethod::RealTime));

        let segments_model = SegmentsModel::new();
        segments_model.build_from_timer(&timer.read().unwrap(), TimingMethod::RealTime);
        let model_store = segments_model.store();
        let model = gtk4::SingleSelection::new(Some(model_store));

        let table = ColumnView::builder()
            .reorderable(false)
            .vscroll_policy(gtk4::ScrollablePolicy::Natural)
            .css_classes(["table"])
            .build();

        context.set_timing_method(TimingMethod::RealTime);

        let scroller = ScrolledWindow::builder()
            .css_classes(["no-background"])
            .kinetic_scrolling(true)
            .hexpand(true)
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .build();
        scroller.set_child(Some(&table));

        let container = GtkBox::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(12)
            .vexpand(true)
            .hexpand(true)
            .build();
        container.append(&scroller);

        let this = Self {
            container,
            table,
            model,
            timer,
            run_snapshot,
            timing_method,
            context,
            segments_model,
        };

        this.table.set_model(Some(&this.model));
        let reference_this = Rc::new(this);
        reference_this.setup_columns();

        let controls = reference_this.build_controls();
        reference_this.container.append(&controls);

        reference_this
    }

    pub fn container(&self) -> &GtkBox {
        &self.container
    }

    pub fn cancel_changes(&self) -> Option<()> {
        self.timer
            .write()
            .unwrap()
            .set_run(self.run_snapshot.clone())
            .ok()
    }

    fn setup_columns(self: &Rc<SegmentsEditor>) {
        let name_column = self.make_name_column();
        let split_time_column = self.clone().make_split_time_column();
        let segment_time_column = self.clone().make_segment_time_column();
        let best_column = self.clone().make_best_segment_column();

        self.table.append_column(&name_column);
        self.table.append_column(&split_time_column);
        self.table.append_column(&segment_time_column);
        self.table.append_column(&best_column);
        {
            let ctx = self.context.clone();
            let weak_this = std::rc::Rc::downgrade(self);
            ctx.connect_local("run-changed", false, move |_values| {
                if let Some(this) = weak_this.upgrade() {
                    this.update_data_model();
                }
                None
            });
        }
        {
            let ctx = self.context.clone();
            let weak_this = std::rc::Rc::downgrade(self);
            let ctx_for_closure = ctx.clone();
            ctx.connect_local("timing-method-changed", false, move |_values| {
                if let Some(this) = weak_this.upgrade() {
                    let method = ctx_for_closure.timing_method();
                    *this.timing_method.write().unwrap() = method;
                    this.update_data_model();
                }
                None
            });
        }
    }

    fn update_data_model(&self) {
        let timer = self.timer.read().unwrap();
        let method = *self.timing_method.read().unwrap();
        self.segments_model.refresh_from_timer(&timer, method);
    }

    fn make_name_column(&self) -> ColumnViewColumn {
        let col = ColumnViewColumn::builder().title("Segment Name").build();
        let factory = gtk4::SignalListItemFactory::new();

        let context = self.context.clone();
        let model = self.model.clone();

        factory.connect_setup(move |_, list_item| {
            let cell = list_item.downcast_ref::<gtk4::ColumnViewCell>().unwrap();
            let entry = gtk4::Entry::builder().hexpand(true).build();
            cell.set_child(Some(&entry));

            SegmentsEditor::setup_name_cell_common(cell, &entry, &model, &context);
        });
        factory.connect_bind(|_, list_item| {
            let cell = list_item.downcast_ref::<gtk4::ColumnViewCell>().unwrap();
            let entry = cell.child().unwrap().downcast::<gtk4::Entry>().unwrap();

            if let Some(item) = cell.item()
                && let Ok(row) = item.downcast::<SegmentRow>()
            {
                entry.set_text(&row.name());
                row.bind_property("name", &entry, "text")
                    .flags(glib::BindingFlags::SYNC_CREATE)
                    .build();
            }
        });
        col.set_factory(Some(&factory));
        col
    }

    fn make_split_time_column(self: Rc<Self>) -> ColumnViewColumn {
        let col = ColumnViewColumn::builder().title("Split Time").build();
        let factory = gtk4::SignalListItemFactory::new();

        let self_shared = Rc::clone(&self);

        factory.connect_setup(move |_, list_item| {
            let cell = list_item.downcast_ref::<gtk4::ColumnViewCell>().unwrap();
            let entry = gtk4::Entry::builder().hexpand(true).build();
            cell.set_child(Some(&entry));

            SegmentsEditor::setup_time_cell_common(
                cell,
                &entry,
                &self_shared,
                "split-time".to_string(),
                SegmentsEditor::commit_split_time,
            );
        });
        factory.connect_bind(move |_, list_item| {
            let cell = list_item.downcast_ref::<gtk4::ColumnViewCell>().unwrap();
            let entry = cell.child().unwrap().downcast::<gtk4::Entry>().unwrap();

            if let Some(item) = cell.item()
                && let Ok(row) = item.downcast::<SegmentRow>()
            {
                entry.set_text(&row.split_time());
                row.bind_property("split-time", &entry, "text")
                    .flags(glib::BindingFlags::SYNC_CREATE)
                    .build();
            }
        });
        col.set_factory(Some(&factory));
        col
    }

    fn make_segment_time_column(self: Rc<Self>) -> ColumnViewColumn {
        let col = ColumnViewColumn::builder().title("Segment Time").build();
        let factory = gtk4::SignalListItemFactory::new();

        let self_shared = Rc::clone(&self);

        factory.connect_setup(move |_, list_item| {
            let cell = list_item.downcast_ref::<gtk4::ColumnViewCell>().unwrap();
            let entry = gtk4::Entry::builder().hexpand(true).build();
            cell.set_child(Some(&entry));

            SegmentsEditor::setup_time_cell_common(
                cell,
                &entry,
                &self_shared,
                "segment-time".to_string(),
                SegmentsEditor::commit_segment_time,
            );
        });
        factory.connect_bind(move |_, list_item| {
            let cell = list_item.downcast_ref::<gtk4::ColumnViewCell>().unwrap();
            let entry = cell.child().unwrap().downcast::<gtk4::Entry>().unwrap();

            if let Some(item) = cell.item()
                && let Ok(row) = item.downcast::<SegmentRow>()
            {
                entry.set_text(&row.segment_time());
                row.bind_property("segment-time", &entry, "text")
                    .flags(glib::BindingFlags::SYNC_CREATE)
                    .build();
            }
        });
        col.set_factory(Some(&factory));
        col
    }

    fn make_best_segment_column(self: Rc<Self>) -> ColumnViewColumn {
        let col = ColumnViewColumn::builder().title("Best Segment").build();
        let factory = gtk4::SignalListItemFactory::new();

        let self_shared = Rc::clone(&self);

        factory.connect_setup(move |_, list_item| {
            let cell = list_item.downcast_ref::<gtk4::ColumnViewCell>().unwrap();
            let entry = gtk4::Entry::builder().hexpand(true).build();
            cell.set_child(Some(&entry));

            SegmentsEditor::setup_time_cell_common(
                cell,
                &entry,
                &self_shared,
                "best".to_string(),
                SegmentsEditor::commit_best_time,
            );
        });
        factory.connect_bind(|_, list_item| {
            let cell = list_item.downcast_ref::<gtk4::ColumnViewCell>().unwrap();
            let entry = cell.child().unwrap().downcast::<gtk4::Entry>().unwrap();

            if let Some(item) = cell.item()
                && let Ok(row) = item.downcast::<SegmentRow>()
            {
                entry.set_text(&row.best());
                row.bind_property("best", &entry, "text")
                    .flags(glib::BindingFlags::SYNC_CREATE)
                    .build();
            }
        });
        col.set_factory(Some(&factory));
        col
    }

    // Set standardized handlers for the name column
    fn setup_name_cell_common(
        cell: &gtk4::ColumnViewCell,
        entry: &gtk4::Entry,
        model: &gtk4::SingleSelection,
        context: &EditorContext,
    ) {
        // Apply name on unfocus and select on focus
        let cell_binding = cell.clone();
        let model_binding = model.clone();
        let context_binding = context.clone();
        entry.connect_notify_local(Some("has-focus"), move |e, _| {
            let focused = e.first_child().unwrap().has_focus();
            if focused {
                // Select the corresponding SegmentRow
                if let Some(item) = cell_binding.item()
                    && let Some(row) = item.downcast_ref::<SegmentRow>()
                {
                    let index = row.index() as usize;
                    model_binding.select_item(index as u32, true);
                }
            } else {
                // Commit name change on unfocus
                if let Some(item) = cell_binding.item()
                    && let Some(row) = item.downcast_ref::<SegmentRow>()
                {
                    let index = row.index() as usize;
                    let value = e.text().to_string();
                    let () = context_binding.set_segment_name(index, value);
                }
            }
        });
    }

    // Sets standardized handlers for time columns (Split/Segment/Best)
    // - Validates on change (adds/removes "error" CSS class)
    // - Commits on unfocus and refreshes the model
    // - Selects row on focus
    fn setup_time_cell_common(
        cell: &gtk4::ColumnViewCell,
        entry: &gtk4::Entry,
        editor: &Rc<SegmentsEditor>,
        property_name: String,
        commit: fn(&EditorContext, usize, i64),
    ) {
        // Validation while typing
        entry.connect_changed(move |e| {
            e.remove_css_class("error");
            let value = e.text().to_string();

            let dur = parse_hms(&value);
            if dur.is_err() || dur.as_ref().ok().unwrap().is_negative() {
                e.add_css_class("error");
            }
        });

        // Apply change on unfocus and refresh model; select row on focus
        let self_binding = editor.clone();
        let cell_binding = cell.clone();
        let context_binding = editor.context.clone();
        entry.connect_notify_local(Some("has-focus"), move |e, _| {
            let focused = e.first_child().unwrap().has_focus();
            if let Some(item) = cell_binding.item()
                && let Some(row) = item.downcast_ref::<SegmentRow>()
            {
                if focused {
                    // Select the corresponding SegmentRow
                    let index = row.index() as usize;
                    self_binding.model.select_item(index as u32, true);
                } else {
                    // Commit value if valid and if different from before
                    let value = e.text().to_string();
                    let is_different = match property_name.as_str() {
                        "split-time" => value != row.split_time(),
                        "segment-time" => value != row.segment_time(),
                        "best" => value != row.best(),
                        _ => false,
                    };
                    if let Ok(dur) = parse_hms(&value)
                        && !dur.is_negative()
                        && let Some(item) = cell_binding.item()
                        && let Some(row) = item.downcast_ref::<SegmentRow>()
                        && is_different
                    {
                        let index = row.index() as usize;
                        let ms = dur.whole_milliseconds();
                        commit(&context_binding, index, ms as i64);
                    }
                }
            }
        });
    }

    // Small helpers to bridge into EditorContext
    fn commit_split_time(ctx: &EditorContext, index: usize, ms: i64) {
        ctx.set_split_time_ms(index, ms);
    }
    fn commit_segment_time(ctx: &EditorContext, index: usize, ms: i64) {
        ctx.set_segment_time_ms(index, ms);
    }
    fn commit_best_time(ctx: &EditorContext, index: usize, ms: i64) {
        ctx.set_best_time_ms(index, ms);
    }

    // Builds the editor controls (Move split up/down, Add split above, Remove split)
    fn build_controls(&self) -> gtk4::Box {
        let controls = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .valign(gtk4::Align::Fill)
            .homogeneous(true)
            .spacing(6)
            .width_request(40)
            .build();

        let move_group = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(0)
            .homogeneous(true)
            .valign(gtk4::Align::Fill)
            .css_classes(["button-group"])
            .build();
        {
            let move_up_button = gtk4::Button::builder()
                .icon_name("move-up-symbolic")
                .build();
            {
                let context = self.context.clone();
                let model_binding = self.model.clone();
                move_up_button.connect_clicked(move |_| {
                    context
                        .move_segment(model_binding.selected() as usize, SegmentMoveDirection::Up);
                    model_binding
                        .set_selected(std::cmp::max(model_binding.selected().saturating_sub(1), 0));
                });
            }
            let move_down_button = gtk4::Button::builder()
                .icon_name("move-down-symbolic")
                .build();
            {
                let context = self.context.clone();
                let model_binding = self.model.clone();
                move_down_button.connect_clicked(move |_| {
                    context.move_segment(
                        model_binding.selected() as usize,
                        SegmentMoveDirection::Down,
                    );
                    model_binding.set_selected(std::cmp::min(
                        model_binding.selected() + 1,
                        context.timer().read().unwrap().run().segments().len() as u32 - 1, // At least one segment will be present
                    ));
                });
            }
            move_group.append(&move_up_button);
            move_group.append(&move_down_button);
        }

        let add_group = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(0)
            .homogeneous(true)
            .valign(gtk4::Align::Fill)
            .css_classes(["button-group"])
            .build();
        {
            let add_split_up_button = gtk4::Button::builder()
                .icon_name("add-above-symbolic")
                .build();
            {
                let context = self.context.clone();
                let model_binding = self.model.clone();
                add_split_up_button.connect_clicked(move |_| {
                    let selected = model_binding.selected(); // We need to capture this before adding, as it will reset to 0
                    context.add_segment(selected as usize, SegmentMoveDirection::Up);
                    // We do not move the selection, as the new segment is added where the current one was
                    model_binding.set_selected(selected);
                });
            }
            let add_split_down_button = gtk4::Button::builder()
                .icon_name("add-below-symbolic")
                .build();
            {
                let context = self.context.clone();
                let model_binding = self.model.clone();
                add_split_down_button.connect_clicked(move |_| {
                    let selected = model_binding.selected(); // We need to capture this before adding
                    context.add_segment(selected as usize, SegmentMoveDirection::Down);
                    model_binding.set_selected(std::cmp::min(
                        selected + 1,
                        context.timer().read().unwrap().run().segments().len() as u32 - 1, // At least one segment will be present
                    ));
                });
            }
            add_group.append(&add_split_up_button);
            add_group.append(&add_split_down_button);
        }

        let remove_split_button = gtk4::Button::builder()
            .icon_name("user-trash-symbolic")
            .css_classes(["destructive-action"])
            .build();
        {
            let context = self.context.clone();
            let model_binding = self.model.clone();
            remove_split_button.connect_clicked(move |_| {
                let selected = model_binding.selected();
                context.remove_segment(selected as usize);
                // We restore the selection
                model_binding.set_selected(std::cmp::min(
                    selected,
                    context.timer().read().unwrap().run().segments().len() as u32 - 1, // At least one segment will be present
                ));
            });
        }

        controls.append(&move_group);
        controls.append(&add_group);
        controls.append(&remove_split_button);
        controls
    }
}

#[cfg(test)]
impl SegmentsEditor {
    // Test-only helpers to inspect internal model and context without touching UI widgets.
    pub fn __test_items(&self) -> Vec<SegmentRow> {
        let mut out = Vec::new();
        if let Some(model) = self.table.model() {
            for i in 0..model.n_items() {
                if let Some(obj) = model.item(i) {
                    if let Ok(row) = obj.downcast::<SegmentRow>() {
                        out.push(row);
                    }
                }
            }
        }
        out
    }

    pub fn __test_context(&self) -> EditorContext {
        self.context.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use livesplit_core::{Run, Segment, Time, TimeSpan, Timer, TimingMethod};
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn gtk_test_init() {
        INIT.call_once(|| {
            gtk4::init().expect("Failed to init GTK");
        });
    }

    fn make_timer_with_run(mut run: Run) -> Arc<RwLock<Timer>> {
        Arc::new(RwLock::new(Timer::new(run).expect("timer")))
    }

    fn time_both(rt_secs: i64, gt_secs: i64) -> Time {
        Time::new()
            .with_real_time(Some(TimeSpan::from_seconds(rt_secs as f64)))
            .with_game_time(Some(TimeSpan::from_seconds(gt_secs as f64)))
    }

    #[gtk4::test]
    fn cancel_changes_restores_snapshot() {
        gtk_test_init();
        // Initial run snapshot
        let mut run = Run::new();
        run.set_game_name("Game");
        run.set_category_name("Any%");
        run.push_segment(Segment::new("S1"));
        let timer = make_timer_with_run(run);

        // Build editor (takes snapshot internally)
        let context = EditorContext::new(Arc::clone(&timer), None);
        let editor = SegmentsEditor::new(context);

        // Mutate the timer's run
        {
            let mut t = timer.write().unwrap();
            let mut mutated = t.run().clone();
            mutated.set_game_name("Changed");
            mutated.set_category_name("ChangedCat");
            mutated.push_segment(Segment::new("S2"));
            t.set_run(mutated).expect("set mutated run");
        }

        // Cancel changes -> should restore to the original snapshot
        {
            let editor2 = Rc::clone(&editor);
            editor2.cancel_changes().expect("cancel to succeed");
        }

        // Assert restored
        let t = timer.read().unwrap();
        assert_eq!(t.run().game_name(), "Game");
        assert_eq!(t.run().category_name(), "Any%");
        assert_eq!(t.run().segments().len(), 1);
        assert_eq!(t.run().segments()[0].name(), "S1");
    }

    #[gtk4::test]
    fn timing_method_change_refreshes_model() {
        gtk_test_init();
        // Run with one segment; PB split has different RT vs GT
        let mut run = Run::new();
        run.set_game_name("Game");
        run.set_category_name("Any%");
        let mut s1 = Segment::new("S1");
        s1.set_personal_best_split_time(time_both(10, 20));
        run.push_segment(s1);

        let timer = make_timer_with_run(run);
        let context = EditorContext::new(Arc::clone(&timer), None);
        let editor = SegmentsEditor::new(context);

        // Initially RealTime -> expect 10.000
        {
            let items = editor.__test_items();
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].split_time(), "10.000");
        }

        // Switch to GameTime via the context signal; this should refresh the model
        editor
            .__test_context()
            .set_timing_method(TimingMethod::GameTime);

        // Now expect 20.000 reflected in the model
        {
            let items = editor.__test_items();
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].split_time(), "20.000");
        }
    }
}
