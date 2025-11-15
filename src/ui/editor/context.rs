use std::cell::{Cell, RefCell};
use std::sync::{Arc, OnceLock, RwLock};

use glib::prelude::*;
use glib::subclass::prelude::*;

use glib::{Properties, subclass::signal::Signal};
use livesplit_core::{RunEditor, TimeSpan, Timer, TimingMethod};

mod imp {
    use super::{
        Arc, Cell, DerivedObjectProperties, ObjectImpl, ObjectImplExt, ObjectSubclass, OnceLock,
        Properties, RefCell, RwLock, Signal, Timer, TimingMethod,
    };

    #[derive(Properties)]
    #[properties(wrapper_type = super::EditorContext)]
    pub struct EditorContext {
        // Shared timer instance set by EditorContext::new(...)
        pub timer: RefCell<Arc<RwLock<Timer>>>,
        // Timing method used for edits: 0 = RealTime, 1 = GameTime
        pub timing_method: Cell<i32>,
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
    pub fn new(timer: Arc<RwLock<Timer>>) -> Self {
        let obj: Self = glib::Object::new();
        obj.imp().timer.replace(timer);
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
    }
}
