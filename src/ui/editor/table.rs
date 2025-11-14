use livesplit_core::{Run, RunEditor, TimeSpan, Timer, TimingMethod};
use std::sync::{Arc, RwLock};

use glib::subclass::prelude::*;
use gtk4::{ColumnView, ColumnViewColumn, gio::ListStore, prelude::*};

use glib::Properties;
use std::cell::{OnceCell, RefCell};

use tracing::debug;

use crate::formatters::time::{TimeFormat, parse_hms};
use crate::ui::editor::row::SegmentRow;
use crate::utils::comparisons::{
    previous_split_combined_gold_and_prev_comparison, segment_comparison_time,
};

pub struct SegmentsEditor {
    table: ColumnView,
    timer: Arc<RwLock<Timer>>,
    run_snapshot: Run, // Snapshot of the timer at the moment of opening the editor
    timing_method: Arc<RwLock<TimingMethod>>,
}

impl SegmentsEditor {
    pub fn new(timer: Arc<RwLock<Timer>>) -> Self {
        let run_snapshot = timer.read().unwrap().run().clone();

        let timing_method = Arc::new(RwLock::new(TimingMethod::RealTime));

        let table = ColumnView::builder().vexpand(true).build();
        let mut this = Self {
            table,
            timer,
            run_snapshot,
            timing_method,
        };

        let model = this.create_data_model();
        this.table
            .set_model(Some(&gtk4::SingleSelection::new(Some(model))));

        let name_column = this.make_name_column();
        let split_time_column = this.make_split_time_column();
        let segment_time_column = this.make_segment_time_column();
        let best_column = this.make_best_segment_column();

        this.table.append_column(&name_column);
        this.table.append_column(&split_time_column);
        this.table.append_column(&segment_time_column);
        this.table.append_column(&best_column);

        this
    }

    pub fn table(&self) -> &ColumnView {
        &self.table
    }

    pub fn cancel_changes(&mut self) -> Option<()> {
        self.timer
            .write()
            .unwrap()
            .set_run(self.run_snapshot.clone())
            .ok()
    }

    pub fn create_data_model(&self) -> ListStore {
        let mut time_parser = TimeFormat::new(true, true, true, true, 3, false);

        let timer = self.timer.read().unwrap();

        let store = ListStore::new::<SegmentRow>();

        let segments = self.run_snapshot.segments();

        for (index, segment) in segments.iter().enumerate() {
            let mut last_non_skipped: Option<usize> = None;
            if index > 0 {
                for k in (0..index).rev() {
                    if segments[k]
                        .comparison_timing_method(
                            "Personal Best",
                            self.timing_method.read().unwrap().to_owned(),
                        )
                        .unwrap_or_default()
                        .to_duration()
                        != time::Duration::ZERO
                    {
                        last_non_skipped = Some(k);
                        break;
                    }
                }
            }

            let previous_comparison_duration = last_non_skipped.map_or(time::Duration::ZERO, |k| {
                segments[k]
                    .comparison_timing_method(
                        "Personal Best",
                        self.timing_method.read().unwrap().to_owned(),
                    )
                    .unwrap_or_default()
                    .to_duration()
            });

            debug!(
                "Previous comparison duration for segment {}: {:?}",
                index, previous_comparison_duration
            );

            let name = segment.name().to_string();
            let split_time = segment
                .comparison_timing_method(
                    "Personal Best",
                    self.timing_method.read().unwrap().to_owned(),
                )
                .map_or(String::new(), |t| time_parser.format_time_span(&t));
            let segment_time = segment
                .comparison_timing_method(
                    "Personal Best",
                    self.timing_method.read().unwrap().to_owned(),
                )
                .map_or(String::new(), |t| {
                    let dur = t.to_duration().saturating_sub(previous_comparison_duration);
                    time_parser.format_duration(&dur)
                });
            let best = segment
                .comparison_timing_method("Best Segments", timer.current_timing_method())
                .map_or(String::new(), |t| time_parser.format_time_span(&t));

            let row = SegmentRow::new(index as u32, name, split_time, segment_time, best);
            store.append(&row);
        }

        store
    }

    fn make_name_column(&mut self) -> ColumnViewColumn {
        let col = ColumnViewColumn::builder().title("Segment Name").build();
        let factory = gtk4::SignalListItemFactory::new();

        let timer = self.timer.clone();

        factory.connect_setup(move |_, list_item| {
            let cell = list_item.downcast_ref::<gtk4::ColumnViewCell>().unwrap();
            let entry = gtk4::Entry::builder()
                .hexpand(true)
                .margin_start(12)
                .margin_end(12)
                .build();
            cell.set_child(Some(&entry));

            let timer_binding = timer.clone();
            let cell_binding = cell.clone();
            entry.connect_changed(move |e| {
                let value = e.text().to_string();
                let mut timer = timer_binding.write().unwrap();
                let mut run = timer.run().clone();

                if let Some(item) = cell_binding.item()
                    && let Ok(row) = item.downcast::<SegmentRow>()
                {
                    let index = row.index() as usize;
                    if index < run.segments().len() {
                        run.segments_mut()[index].set_name(value);
                        assert!(timer.set_run(run).is_ok());
                    }
                }
            });
        });
        factory.connect_bind(|_, list_item| {
            let cell = list_item
                .to_owned()
                .downcast::<gtk4::ColumnViewCell>()
                .unwrap();
            if let Some(child) = cell.child() {
                let entry = child.downcast_ref::<gtk4::Entry>().unwrap();
                if let Some(item) = cell.item()
                    && let Ok(row) = item.downcast::<SegmentRow>()
                {
                    let current: String = row.property("name");
                    entry.set_text(&current);
                }
            }
        });
        col.set_factory(Some(&factory));
        col
    }

    fn make_split_time_column(&mut self) -> ColumnViewColumn {
        let col = ColumnViewColumn::builder().title("Split Time").build();
        let factory = gtk4::SignalListItemFactory::new();

        let timer = self.timer.clone();
        let timing_method = self.timing_method.clone();

        factory.connect_setup(move |_, list_item| {
            let cell = list_item.downcast_ref::<gtk4::ColumnViewCell>().unwrap();
            let entry = gtk4::Entry::builder()
                .hexpand(true)
                .margin_start(12)
                .margin_end(12)
                .build();
            cell.set_child(Some(&entry));

            let timer_binding = timer.clone();
            let cell_binding = cell.clone();
            let timing_method_binding = timing_method.clone();
            entry.connect_changed(move |e| {
                e.remove_css_class("error");
                let value = e.text().to_string();

                // Value validation
                let dur = parse_hms(&value);
                if dur.is_err() || dur.as_ref().ok().unwrap().is_negative() {
                    e.add_css_class("error");
                    return;
                }

                let mut timer = timer_binding.write().unwrap();
                let mut run = timer.run().clone();

                if let Some(item) = cell_binding.item()
                    && let Ok(row) = item.downcast::<SegmentRow>()
                {
                    let index = row.index() as usize;
                    if index < run.segments().len() {
                        *run.segment_mut(index).personal_best_split_time_mut() = run
                            .segment_mut(index)
                            .personal_best_split_time_mut()
                            .with_timing_method(
                                timing_method_binding.read().unwrap().to_owned(),
                                Some(TimeSpan::from_milliseconds(
                                    dur.as_ref().ok().unwrap().whole_milliseconds() as f64,
                                )),
                            );
                        assert!(timer.set_run(run).is_ok());
                    }
                }
            });
        });
        factory.connect_bind(|_, list_item| {
            let cell = list_item
                .to_owned()
                .downcast::<gtk4::ColumnViewCell>()
                .unwrap();
            if let Some(child) = cell.child() {
                let entry = child.downcast_ref::<gtk4::Entry>().unwrap();
                if let Some(item) = cell.item()
                    && let Ok(row) = item.downcast::<SegmentRow>()
                {
                    let current: String = row.property("split_time");
                    entry.set_text(&current);
                }
            }
        });
        col.set_factory(Some(&factory));
        col
    }

    fn make_segment_time_column(&mut self) -> ColumnViewColumn {
        let col = ColumnViewColumn::builder().title("Segment Time").build();
        let factory = gtk4::SignalListItemFactory::new();

        let timer = self.timer.clone();
        let timing_method = self.timing_method.clone();

        factory.connect_setup(move |_, list_item| {
            let cell = list_item.downcast_ref::<gtk4::ColumnViewCell>().unwrap();
            let entry = gtk4::Entry::builder()
                .hexpand(true)
                .margin_start(12)
                .margin_end(12)
                .build();
            cell.set_child(Some(&entry));

            let timer_binding = timer.clone();
            let cell_binding = cell.clone();
            let timing_method_binding = timing_method.clone();
            entry.connect_changed(move |e| {
                e.remove_css_class("error");
                let value = e.text().to_string();

                // Value validation
                let dur = parse_hms(&value);
                if dur.is_err() || dur.as_ref().ok().unwrap().is_negative() {
                    e.add_css_class("error");
                    return;
                }

                // Here the best way to do this is with a RunEditor, which can access and edit the segment time directly
                let mut timer = timer_binding.write().unwrap();
                let mut run_editor = RunEditor::new(timer.run().to_owned()).ok().unwrap();

                if let Some(item) = cell_binding.item()
                    && let Ok(row) = item.downcast::<SegmentRow>()
                {
                    let index = row.index() as usize;
                    if index < run_editor.run().segments().len() {
                        run_editor.select_additionally(index);
                        run_editor
                            .select_timing_method(timing_method_binding.read().unwrap().to_owned());
                        run_editor.active_segment().set_segment_time(Some(
                            TimeSpan::from_milliseconds(
                                dur.as_ref().ok().unwrap().whole_milliseconds() as f64,
                            ),
                        ));
                        run_editor.unselect(index);

                        assert!(timer.set_run(run_editor.close()).is_ok());
                    }
                }
            });
        });
        factory.connect_bind(|_, list_item| {
            let cell = list_item
                .to_owned()
                .downcast::<gtk4::ColumnViewCell>()
                .unwrap();
            if let Some(child) = cell.child() {
                let entry = child.downcast_ref::<gtk4::Entry>().unwrap();
                if let Some(item) = cell.item()
                    && let Ok(row) = item.downcast::<SegmentRow>()
                {
                    let current: String = row.property("segment_time");
                    entry.set_text(&current);
                }
            }
        });
        col.set_factory(Some(&factory));
        col
    }

    fn make_best_segment_column(&mut self) -> ColumnViewColumn {
        let col = ColumnViewColumn::builder().title("Best Segment").build();
        let factory = gtk4::SignalListItemFactory::new();

        let timer = self.timer.clone();
        let timing_method = self.timing_method.clone();

        factory.connect_setup(move |_, list_item| {
            let cell = list_item.downcast_ref::<gtk4::ColumnViewCell>().unwrap();
            let entry = gtk4::Entry::builder()
                .hexpand(true)
                .margin_start(12)
                .margin_end(12)
                .build();
            cell.set_child(Some(&entry));

            let timer_binding = timer.clone();
            let cell_binding = cell.clone();
            let timing_method_binding = timing_method.clone();
            entry.connect_changed(move |e| {
                e.remove_css_class("error");
                let value = e.text().to_string();

                // Value validation
                let dur = parse_hms(&value);
                if dur.is_err() || dur.as_ref().ok().unwrap().is_negative() {
                    e.add_css_class("error");
                    return;
                }

                let mut timer = timer_binding.write().unwrap();
                let mut run = timer.run().clone();

                if let Some(item) = cell_binding.item()
                    && let Ok(row) = item.downcast::<SegmentRow>()
                {
                    let index = row.index() as usize;
                    if index < run.segments().len() {
                        *run.segment_mut(index).best_segment_time_mut() = run
                            .segment_mut(index)
                            .best_segment_time_mut()
                            .with_timing_method(
                                timing_method_binding.read().unwrap().to_owned(),
                                Some(TimeSpan::from_milliseconds(
                                    dur.as_ref().ok().unwrap().whole_milliseconds() as f64,
                                )),
                            );
                        assert!(timer.set_run(run).is_ok());
                    }
                }
            });
        });
        factory.connect_bind(|_, list_item| {
            let cell = list_item
                .to_owned()
                .downcast::<gtk4::ColumnViewCell>()
                .unwrap();
            if let Some(child) = cell.child() {
                let entry = child.downcast_ref::<gtk4::Entry>().unwrap();
                if let Some(item) = cell.item()
                    && let Ok(row) = item.downcast::<SegmentRow>()
                {
                    let current: String = row.property("best");
                    entry.set_text(&current);
                }
            }
        });
        col.set_factory(Some(&factory));
        col
    }
}
