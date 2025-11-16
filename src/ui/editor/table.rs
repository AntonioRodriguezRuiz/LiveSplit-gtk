use livesplit_core::{Run, Timer, TimingMethod};
use std::rc::Rc;
use std::sync::{Arc, RwLock};

use gtk4::{ColumnView, ColumnViewColumn, ScrolledWindow, prelude::*};

use crate::formatters::time::parse_hms;
use crate::ui::editor::row::SegmentRow;
use crate::ui::editor::{EditorContext, SegmentsModel};

pub struct SegmentsEditor {
    scroller: ScrolledWindow,
    table: ColumnView,
    model: gtk4::SingleSelection,
    timer: Arc<RwLock<Timer>>,
    run_snapshot: Run, // Snapshot of the timer at the moment of opening the editor
    timing_method: Arc<RwLock<TimingMethod>>,
    context: EditorContext,
    segments_model: SegmentsModel,
}

impl SegmentsEditor {
    pub fn new(timer: Arc<RwLock<Timer>>) -> Rc<Self> {
        let run_snapshot = timer.read().unwrap().run().clone();
        let timing_method = Arc::new(RwLock::new(TimingMethod::RealTime));

        let segments_model = SegmentsModel::new();
        segments_model.build_from_timer(&timer.read().unwrap(), TimingMethod::RealTime);
        let model_store = segments_model.store();
        let model = gtk4::SingleSelection::new(Some(model_store));

        let table = ColumnView::builder()
            .vscroll_policy(gtk4::ScrollablePolicy::Natural)
            .reorderable(false)
            .css_classes(["table"])
            .vexpand(false)
            .build();

        let context = EditorContext::new(timer.clone());
        context.set_timing_method(TimingMethod::RealTime);

        let scroller = ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .css_classes(["no-background"])
            .kinetic_scrolling(true)
            .build();

        scroller.set_child(Some(&table));

        let this = Self {
            scroller,
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

        reference_this
    }

    pub fn scroller(&self) -> &ScrolledWindow {
        &self.scroller
    }

    pub fn cancel_changes(&mut self) -> Option<()> {
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
                    self_binding.update_data_model();
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
}
