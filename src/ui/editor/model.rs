use gtk4::gio::ListStore;
use gtk4::prelude::*;
use livesplit_core::{Segment, Timer, TimingMethod};
use time::Duration as TimeDuration;

use crate::formatters::time::TimeFormat;
use crate::ui::editor::row::SegmentRow;

/// SegmentsModel owns the ListStore of SegmentRow and provides methods
/// to build and refresh it from a Timer and a chosen TimingMethod.
///
/// Notes:
/// - `build_from_timer` clears and recreates all rows.
/// - `refresh_from_timer` updates existing rows in place when the segment
///   count matches. If the count differs, it falls back to a full rebuild.
pub struct SegmentsModel {
    store: ListStore, // ListStore<SegmentRow>
}

impl Default for SegmentsModel {
    fn default() -> Self {
        Self::new()
    }
}

impl SegmentsModel {
    /// Creates a new, empty model.
    pub fn new() -> Self {
        Self {
            store: ListStore::new::<SegmentRow>(),
        }
    }

    /// Returns a clone of the underlying ListStore so it can be set as the model.
    pub fn store(&self) -> ListStore {
        self.store.clone()
    }

    /// Clears and repopulates the store from the given Timer and TimingMethod.
    pub fn build_from_timer(&self, timer: &Timer, timing_method: TimingMethod) {
        self.store.remove_all();

        let mut formatter = TimeFormat::new(true, true, true, true, 3, false);
        let segments = timer.run().segments();

        for (index, segment) in segments.iter().enumerate() {
            let (name, split_time, segment_time, best) =
                compute_row_values(timing_method, &mut formatter, segments, index, segment);

            let row = SegmentRow::new(index as u32, name, split_time, segment_time, best);
            self.store.append(&row);
        }
    }

    /// Updates existing rows in-place from the given Timer and TimingMethod.
    ///
    /// If the number of segments differs from the number of rows, this rebuilds the model.
    pub fn refresh_from_timer(&self, timer: &Timer, timing_method: TimingMethod) {
        let segments = timer.run().segments();
        let row_count = self.store.n_items() as usize;

        if row_count != segments.len() {
            // Segment count changed; rebuild
            self.build_from_timer(timer, timing_method);
            return;
        }

        let mut formatter = TimeFormat::new(true, true, true, true, 3, false);

        for (index, item) in self.store.iter::<SegmentRow>().enumerate() {
            if let Ok(row) = item
                && index < segments.len()
            {
                let segment = &segments[index];
                let (name, split_time, segment_time, best) =
                    compute_row_values(timing_method, &mut formatter, segments, index, segment);

                row.set_name(name);
                row.set_split_time(split_time);
                row.set_segment_time(segment_time);
                row.set_best(best);
            }
        }
    }
}

/// Computes the display values for a single row, mirroring the logic used by the editor table.
///
/// - name: segment name
/// - split_time: segment's comparison time ("Personal Best") formatted
/// - segment_time: delta between this segment's PB split and the last non-skipped PB split
/// - best: delta between this segment's "Best Segments" and the last non-zero best segment
fn compute_row_values(
    timing_method: TimingMethod,
    time_parser: &mut TimeFormat,
    segments: &[Segment],
    index: usize,
    segment: &Segment,
) -> (String, String, String, String) {
    // Find last non-skipped PB split
    let mut last_non_skipped: Option<usize> = None;
    if index > 0 {
        for k in (0..index).rev() {
            if segments[k]
                .comparison_timing_method("Personal Best", timing_method)
                .unwrap_or_default()
                .to_duration()
                != TimeDuration::ZERO
            {
                last_non_skipped = Some(k);
                break;
            }
        }
    }

    // Find last non-zero best segment
    let mut last_gold: Option<usize> = None;
    if index > 0 {
        for k in (0..index).rev() {
            if segments[k]
                .comparison_timing_method("Best Segments", timing_method)
                .unwrap_or_default()
                .to_duration()
                != TimeDuration::ZERO
            {
                last_gold = Some(k);
                break;
            }
        }
    }

    let previous_comparison_duration = last_non_skipped.map_or(TimeDuration::ZERO, |k| {
        segments[k]
            .comparison_timing_method("Personal Best", timing_method)
            .unwrap_or_default()
            .to_duration()
    });

    let name = segment.name().to_string();

    let split_time = segment
        .comparison_timing_method("Personal Best", timing_method)
        .map_or(String::new(), |t| time_parser.format_time_span(&t));

    let segment_time = segment
        .comparison_timing_method("Personal Best", timing_method)
        .map_or(String::new(), |t| {
            let dur = t.to_duration().saturating_sub(previous_comparison_duration);
            time_parser.format_duration(&dur)
        });

    let best_delta = segment
        .comparison_timing_method("Best Segments", timing_method)
        .unwrap_or_default()
        .to_duration()
        .saturating_sub({
            last_gold.map_or(TimeDuration::ZERO, |k| {
                segments[k]
                    .comparison_timing_method("Best Segments", timing_method)
                    .unwrap_or_default()
                    .to_duration()
            })
        });

    let best_formatted = time_parser.format_duration(&best_delta);
    (name, split_time, segment_time, best_formatted)
}
