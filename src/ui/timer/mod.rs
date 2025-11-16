pub mod body;
pub mod footer;
pub mod header;

use crate::config::Config;
use crate::ui::timer::body::TimerBody;
use crate::ui::timer::footer::TimerFooter;
use crate::ui::timer::header::TimerHeader;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, RwLock};

use core::time::Duration;

use adw::Clamp;
use adw::prelude::*;
use gtk4::{Align, Box as GtkBox, Orientation::Vertical};

use crate::context::TuxSplitContext;
use livesplit_core::Timer;

pub struct TuxSplitTimer {
    timer: Arc<RwLock<Timer>>,
    config: Arc<RwLock<Config>>,
    clamp: Clamp,
    header: Rc<RefCell<TimerHeader>>,
    body: Rc<RefCell<TimerBody>>,
    footer: Rc<RefCell<TimerFooter>>,
    refresh_source: Option<glib::SourceId>,
}

impl TuxSplitTimer {
    /// Create the timer widget (header/body/footer composed) but does NOT start refresh loop.
    pub fn new(
        timer: Arc<RwLock<Timer>>,
        config: Arc<RwLock<Config>>,
        ctx: Arc<TuxSplitContext>,
    ) -> Self {
        let clamp = Clamp::builder().maximum_size(900).build();

        let container = GtkBox::builder()
            .orientation(Vertical)
            .valign(Align::Center)
            .halign(Align::Fill)
            .hexpand(true)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .spacing(20)
            .build();

        let header = Rc::new(RefCell::new(TimerHeader::new(&timer.read().unwrap())));

        let mut cfg_write = config.write().unwrap();
        let body = Rc::new(RefCell::new(TimerBody::new(
            &timer.read().unwrap(),
            &mut cfg_write,
        )));
        let footer = Rc::new(RefCell::new(TimerFooter::new(
            &timer.read().unwrap(),
            &mut cfg_write,
            body.borrow().list(),
            body.borrow().last_segment_list(),
        )));
        drop(cfg_write);

        container.append(header.borrow().container());
        container.append(body.borrow().container());
        container.append(footer.borrow().container());

        clamp.set_child(Some(&container));

        {
            // Connect global run-changed to force a rebuild of segment rows.
            let ctx_binding = ctx.clone();
            let timer_binding = timer.clone();
            let config_binding = config.clone();
            let body_binding = body.clone();
            ctx_binding.connect_local("run-changed", false, move |_| {
                let t = timer_binding.read().unwrap();
                let mut c = config_binding.write().unwrap();
                body_binding.borrow_mut().refresh(&t, &mut c, true);
                None
            });
        }

        Self {
            timer,
            config,
            clamp,
            header,
            body,
            footer,
            refresh_source: None,
        }
    }

    pub fn clamped(&self) -> &Clamp {
        &self.clamp
    }

    pub fn start_refresh_loop(&mut self) {
        if self.refresh_source.is_some() {
            return; // Already running
        }

        let timer_binding = self.timer.clone();
        let config_binding = self.config.clone();
        let header_binding = self.header.clone();
        let body_binding = self.body.clone();
        let footer_binding = self.footer.clone();

        let source_id = glib::timeout_add_local(Duration::from_millis(16), move || {
            let t = timer_binding.read().unwrap();
            let mut c = config_binding.write().unwrap();

            header_binding.borrow_mut().refresh(&t, &mut c);
            body_binding.borrow_mut().refresh(&t, &mut c, false);
            footer_binding.borrow_mut().refresh(&t, &mut c);

            glib::ControlFlow::Continue
        });

        self.refresh_source = Some(source_id);
    }

    pub fn stop_refresh_loop(&mut self) {
        if let Some(id) = self.refresh_source.take() {
            id.remove();
        }
    }
}
