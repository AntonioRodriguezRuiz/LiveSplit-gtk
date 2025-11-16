use std::cell::{Cell, RefCell};
use std::sync::{Arc, OnceLock, RwLock};

use glib::prelude::*;
use glib::subclass::prelude::*;

use glib::{Properties, subclass::signal::Signal};
use livesplit_core::{RunEditor, TimeSpan, Timer, TimingMethod};

use crate::context::TuxSplitContext;

pub enum SegmentMoveDirection {
    Up,
    Down,
}

mod imp {
    use super::{
        Arc, Cell, DerivedObjectProperties, ObjectImpl, ObjectImplExt, ObjectSubclass, OnceLock,
        Properties, RefCell, RwLock, Signal, Timer, TimingMethod,
    };
    use crate::context::TuxSplitContext;

    #[derive(Properties)]
    #[properties(wrapper_type = super::EditorContext)]
    pub struct EditorContext {
        // Shared timer instance set by EditorContext::new(...)
        pub timer: RefCell<Arc<RwLock<Timer>>>,
        // Timing method used for edits: 0 = RealTime, 1 = GameTime
        pub timing_method: Cell<i32>,
        // Global application context used to emit a central run-changed signal
        pub global_ctx: RefCell<Option<Arc<TuxSplitContext>>>,
    }

    impl Default for EditorContext {
        fn default() -> Self {
            let mut run = livesplit_core::Run::new();
            let segment = livesplit_core::Segment::new("Segment");
            run.push_segment(segment);
            let timer = Timer::new(run).expect("Failed to create default Timer");
            Self {
                timer: RefCell::new(Arc::new(RwLock::new(timer))),
                timing_method: Cell::new(0), // Default to RealTime
                global_ctx: RefCell::new(None),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for EditorContext {
        const NAME: &'static str = "EditorContext";
        type Type = super::EditorContext;
        type ParentType = glib::Object;
    }

    impl EditorContext {
        #[inline]
        pub fn method(&self) -> TimingMethod {
            match self.timing_method.get() {
                1 => TimingMethod::GameTime,
                _ => TimingMethod::RealTime,
            }
        }

        #[inline]
        pub fn set_method(&self, method: TimingMethod) {
            self.timing_method.set(match method {
                TimingMethod::RealTime => 0,
                TimingMethod::GameTime => 1,
            });
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for EditorContext {
        fn constructed(&self) {
            self.parent_constructed();
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    // Emitted after a successful mutation to the underlying Run via this context.
                    Signal::builder("run-changed").action().build(),
                    // Emitted whenever the timing method used for edits changes.
                    Signal::builder("timing-method-changed").action().build(),
                ]
            })
        }
    }
}

glib::wrapper! {
    pub struct EditorContext(ObjectSubclass<imp::EditorContext>);
}

impl EditorContext {
    /// Construct a new `EditorContext` bound to the provided Timer.
    pub fn new(timer: Arc<RwLock<Timer>>, global: Option<Arc<TuxSplitContext>>) -> Self {
        let obj: Self = glib::Object::new();
        obj.imp().timer.replace(timer);
        obj.imp().global_ctx.replace(global);
        obj
    }

    /// Returns a clone of the shared timer handle.
    pub fn timer(&self) -> Arc<RwLock<Timer>> {
        self.imp().timer.borrow().clone()
    }

    /// Gets the current timing method used for edits.
    pub fn timing_method(&self) -> TimingMethod {
        self.imp().method()
    }

    /// Sets the current timing method used for edits and emits a change signal if it changed.
    pub fn set_timing_method(&self, method: TimingMethod) {
        let old = self.imp().method();
        self.imp().set_method(method);
        if old as i32 != self.imp().timing_method.get() {
            self.emit_by_name::<()>("timing-method-changed", &[]);
        }
    }

    /// Emits the "run-changed" signal to notify listeners a mutation occurred.
    pub fn emit_run_changed(&self) {
        self.emit_by_name::<()>("run-changed", &[]);
    }

    /// Emits the global context's "run-changed" signal if one is attached.
    fn emit_global_run_changed(&self) {
        if let Some(ctx) = self.imp().global_ctx.borrow().as_ref() {
            ctx.emit_run_changed();
        }
    }

    /// Sets the segment name at `index`. Returns true if the operation succeeded.
    ///
    /// Mirrors the existing behavior in table.rs: clones the run, mutates it,
    /// then sets it back on the timer.
    pub fn set_segment_name(&self, index: usize, name: String) {
        let maybe_timer = self.timer();
        let Ok(mut timer) = maybe_timer.try_write() else {
            return;
        };

        let mut run = timer.run().clone();
        if index >= run.segments().len() {
            return;
        }

        run.segments_mut()[index].set_name(name);
        assert!(timer.set_run(run).is_ok());
        drop(timer);

        self.emit_run_changed();
        self.emit_global_run_changed();
    }

    /// Sets the split time at `index` in milliseconds for the current timing method.
    /// Returns true if the operation succeeded.
    ///
    /// Uses `RunEditor` to set the "Personal Best" comparison time, mirroring table.rs.
    pub fn set_split_time_ms(&self, index: usize, ms: i64) {
        if ms < 0 {
            return;
        }

        let maybe_timer = self.timer();
        let Ok(mut timer) = maybe_timer.try_write() else {
            return;
        };

        let mut run_editor = RunEditor::new(timer.run().to_owned()).ok().unwrap();
        if index >= run_editor.run().segments().len() {
            return;
        }

        run_editor.select_additionally(index);
        run_editor.select_timing_method(self.timing_method());
        run_editor.active_segment().set_comparison_time(
            "Personal Best",
            Some(TimeSpan::from_milliseconds(ms as f64)),
        );
        run_editor.unselect(index);

        assert!(timer.set_run(run_editor.close()).is_ok());
        drop(timer);

        self.emit_run_changed();
        self.emit_global_run_changed();
    }

    /// Sets the segment time at `index` in milliseconds for the current timing method.
    /// Returns true if the operation succeeded.
    ///
    /// Uses `RunEditor.active_segment().set_segment_time()`, mirroring table.rs.
    pub fn set_segment_time_ms(&self, index: usize, ms: i64) {
        if ms < 0 {
            return;
        }

        let maybe_timer = self.timer();
        let Ok(mut timer) = maybe_timer.try_write() else {
            return;
        };

        let mut run_editor = RunEditor::new(timer.run().to_owned()).ok().unwrap();
        if index >= run_editor.run().segments().len() {
            return;
        }

        run_editor.select_additionally(index);
        run_editor.select_timing_method(self.timing_method());
        run_editor
            .active_segment()
            .set_segment_time(Some(TimeSpan::from_milliseconds(ms as f64)));
        run_editor.unselect(index);

        assert!(timer.set_run(run_editor.close()).is_ok());
        drop(timer);

        self.emit_run_changed();
        self.emit_global_run_changed();
    }

    /// Sets the best segment time at `index` in milliseconds for the current timing method.
    /// Returns true if the operation succeeded.
    ///
    /// Mutates the Run directly, mirroring the best segment logic in table.rs.
    pub fn set_best_time_ms(&self, index: usize, ms: i64) {
        if ms < 0 {
            return;
        }

        let maybe_timer = self.timer();
        let Ok(mut timer) = maybe_timer.try_write() else {
            return;
        };

        let mut run = timer.run().clone();
        if index >= run.segments().len() {
            return;
        }

        let method = self.timing_method();
        *run.segment_mut(index).best_segment_time_mut() = run
            .segment_mut(index)
            .best_segment_time_mut()
            .with_timing_method(method, Some(TimeSpan::from_milliseconds(ms as f64)));

        assert!(timer.set_run(run).is_ok());
        drop(timer);

        self.emit_run_changed();
        self.emit_global_run_changed();
    }

    /// Moves a given segment up/down by one position.
    pub fn move_segment(&self, index: usize, direction: SegmentMoveDirection) {
        let maybe_timer = self.timer();
        let Ok(mut timer) = maybe_timer.try_write() else {
            return;
        };

        let run = timer.run().clone();

        let mut run_editor = RunEditor::new(run).ok().unwrap();
        run_editor.select_only(index);

        match direction {
            SegmentMoveDirection::Up => {
                if run_editor.can_move_segments_up() {
                    run_editor.move_segments_up();
                } else {
                    return;
                }
            }
            SegmentMoveDirection::Down => {
                if run_editor.can_move_segments_down() {
                    run_editor.move_segments_down();
                } else {
                    return;
                }
            }
        }

        assert!(timer.set_run(run_editor.close()).is_ok());
        drop(timer);

        self.emit_run_changed();
        self.emit_global_run_changed();
    }

    pub fn add_segment(&self, index: usize, direction: SegmentMoveDirection) {
        let maybe_timer = self.timer();
        let Ok(mut timer) = maybe_timer.try_write() else {
            return;
        };

        let mut run_editor = RunEditor::new(timer.run().to_owned()).ok().unwrap();
        run_editor.select_only(index);

        match direction {
            SegmentMoveDirection::Up => {
                run_editor.insert_segment_above();
            }
            SegmentMoveDirection::Down => {
                run_editor.insert_segment_below();
            }
        }

        assert!(timer.set_run(run_editor.close()).is_ok());
        drop(timer);

        self.emit_run_changed();
        self.emit_global_run_changed();
    }

    pub fn remove_segment(&self, index: usize) {
        let maybe_timer = self.timer();
        let Ok(mut timer) = maybe_timer.try_write() else {
            return;
        };

        let mut run_editor = RunEditor::new(timer.run().to_owned()).ok().unwrap();
        run_editor.select_only(index);

        if run_editor.can_remove_segments() {
            run_editor.remove_segments();
        } else {
            return;
        }

        assert!(timer.set_run(run_editor.close()).is_ok());
        drop(timer);

        self.emit_run_changed();
        self.emit_global_run_changed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use livesplit_core::{Run, Segment, Timer, TimingMethod};
    use std::cell::Cell;
    use std::rc::Rc;

    fn make_timer_with_segments(names: &[&str]) -> Arc<RwLock<Timer>> {
        let mut run = Run::new();
        for &n in names {
            run.push_segment(Segment::new(n));
        }
        Arc::new(RwLock::new(Timer::new(run).expect("timer")))
    }

    #[test]
    fn timing_method_signal_emitted_only_on_change() {
        let timer = make_timer_with_segments(&["A"]);
        let ctx = EditorContext::new(timer, None);

        let counter = Rc::new(Cell::new(0));
        let c2 = counter.clone();
        ctx.connect_local("timing-method-changed", false, move |_v| {
            c2.set(c2.get() + 1);
            None
        });

        // Initial is RealTime: setting RealTime again should not emit
        ctx.set_timing_method(TimingMethod::RealTime);
        assert_eq!(counter.get(), 0);

        // Change to GameTime: should emit once
        ctx.set_timing_method(TimingMethod::GameTime);
        assert_eq!(counter.get(), 1);

        // Same value again: no additional emit
        ctx.set_timing_method(TimingMethod::GameTime);
        assert_eq!(counter.get(), 1);

        // Change back to RealTime: emit again
        ctx.set_timing_method(TimingMethod::RealTime);
        assert_eq!(counter.get(), 2);
    }

    #[test]
    fn set_segment_name_valid_and_out_of_bounds() {
        let timer = make_timer_with_segments(&["A"]);
        let ctx = EditorContext::new(timer.clone(), None);

        let run_changed = Rc::new(Cell::new(0));
        let r2 = run_changed.clone();
        ctx.connect_local("run-changed", false, move |_v| {
            r2.set(r2.get() + 1);
            None
        });

        // Valid update
        ctx.set_segment_name(0, "NewName".to_owned());
        {
            let t = timer.read().unwrap();
            assert_eq!(t.run().segments()[0].name(), "NewName");
        }
        assert_eq!(run_changed.get(), 1);

        // Out of bounds: no change, no signal
        ctx.set_segment_name(5, "Nope".to_owned());
        {
            let t = timer.read().unwrap();
            assert_eq!(t.run().segments()[0].name(), "NewName");
        }
        assert_eq!(run_changed.get(), 1);
    }

    #[test]
    fn split_time_setter_handles_negative_and_updates_rt_only() {
        let timer = make_timer_with_segments(&["A"]);
        let ctx = EditorContext::new(timer.clone(), None);

        // Negative should be ignored
        ctx.set_split_time_ms(0, -10);
        {
            let t = timer.read().unwrap();
            let seg = &t.run().segments()[0];
            assert!(
                seg.comparison_timing_method("Personal Best", TimingMethod::RealTime)
                    .is_none()
            );
            assert!(
                seg.comparison_timing_method("Personal Best", TimingMethod::GameTime)
                    .is_none()
            );
        }

        // Set RT split time to 1234ms
        ctx.set_timing_method(TimingMethod::RealTime);
        ctx.set_split_time_ms(0, 1234);
        {
            let t = timer.read().unwrap();
            let seg = &t.run().segments()[0];
            let rt = seg
                .comparison_timing_method("Personal Best", TimingMethod::RealTime)
                .expect("rt pb");
            assert_eq!(rt.to_duration().whole_milliseconds(), 1234);
            // GT should still be None
            assert!(
                seg.comparison_timing_method("Personal Best", TimingMethod::GameTime)
                    .is_none()
            );
        }

        // Now set GT and update; RT remains
        ctx.set_timing_method(TimingMethod::GameTime);
        ctx.set_split_time_ms(0, 2222);
        {
            let t = timer.read().unwrap();
            let seg = &t.run().segments()[0];
            let gt = seg
                .comparison_timing_method("Personal Best", TimingMethod::GameTime)
                .expect("gt pb");
            assert_eq!(gt.to_duration().whole_milliseconds(), 2222);
            let rt = seg
                .comparison_timing_method("Personal Best", TimingMethod::RealTime)
                .expect("rt pb");
            assert_eq!(rt.to_duration().whole_milliseconds(), 1234);
        }
    }

    #[test]
    fn segment_time_setter_handles_negative_and_updates_selected_method() {
        let timer = make_timer_with_segments(&["A"]);
        let ctx = EditorContext::new(timer.clone(), None);

        // Negative ignored
        ctx.set_segment_time_ms(0, -5);

        // Set RT segment time to 1500ms
        ctx.set_timing_method(TimingMethod::RealTime);
        ctx.set_segment_time_ms(0, 1500);

        // Using RunEditor sets the comparison time for PB at that segment for the selected method
        {
            let t = timer.read().unwrap();
            let seg = &t.run().segments()[0];
            let rt_seg = seg
                .comparison_timing_method("Personal Best", TimingMethod::RealTime)
                .expect("rt seg time");
            assert_eq!(rt_seg.to_duration().whole_milliseconds(), 1500);
        }
    }

    #[test]
    fn best_time_setter_handles_negative_out_of_bounds_and_updates_method() {
        let timer = make_timer_with_segments(&["A"]);
        let ctx = EditorContext::new(timer.clone(), None);

        // Negative ignored
        ctx.set_best_time_ms(0, -1);
        {
            let t = timer.read().unwrap();
            let seg = &t.run().segments()[0];
            assert_eq!(seg.best_segment_time().real_time, None);
            assert_eq!(seg.best_segment_time().game_time, None);
        }

        // Out of bounds ignored (no panic / no change)
        ctx.set_best_time_ms(10, 1000);

        // Set RT best to 3210ms
        ctx.set_timing_method(TimingMethod::RealTime);
        ctx.set_best_time_ms(0, 3210);
        {
            let t = timer.read().unwrap();
            let seg = &t.run().segments()[0];
            let best_rt = seg.best_segment_time().real_time.expect("rt best");
            assert_eq!(best_rt.to_duration().whole_milliseconds(), 3210);
            assert!(seg.best_segment_time().game_time.is_none());
        }

        // Switch to GT and set
        ctx.set_timing_method(TimingMethod::GameTime);
        ctx.set_best_time_ms(0, 4321);
        {
            let t = timer.read().unwrap();
            let seg = &t.run().segments()[0];
            let best_gt = seg.best_segment_time().game_time.expect("gt best");
            assert_eq!(best_gt.to_duration().whole_milliseconds(), 4321);
            let best_rt = seg.best_segment_time().real_time.expect("rt best");
            assert_eq!(best_rt.to_duration().whole_milliseconds(), 3210);
        }
    }

    #[test]
    fn run_changed_signal_emitted_on_successful_mutations_only() {
        let timer = make_timer_with_segments(&["A"]);
        let ctx = EditorContext::new(timer, None);

        let count = Rc::new(Cell::new(0));
        let c2 = count.clone();
        ctx.connect_local("run-changed", false, move |_v| {
            c2.set(c2.get() + 1);
            None
        });

        // Invalid (negative) -> no emit
        ctx.set_split_time_ms(0, -1);
        assert_eq!(count.get(), 0);

        // Valid -> emit
        ctx.set_split_time_ms(0, 1000);
        assert_eq!(count.get(), 1);

        // Invalid index -> no emit
        ctx.set_split_time_ms(10, 100);
        assert_eq!(count.get(), 1);
    }
}
