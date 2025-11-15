use glib::subclass::prelude::*;
use gtk4::prelude::*;

use glib::Properties;
use std::cell::RefCell;

mod imp {
    use super::{
        DerivedObjectProperties, ObjectExt, ObjectImpl, ObjectImplExt, ObjectSubclass, Properties,
        RefCell,
    };

    // Segment Row Object
    #[derive(Default, Properties)]
    #[properties(wrapper_type = super::SegmentRow)]
    pub struct SegmentRow {
        #[property(get, set)]
        index: RefCell<u32>,
        #[property(get, set)]
        pub name: RefCell<String>,
        #[property(get, set)]
        pub split_time: RefCell<String>,
        #[property(get, set)]
        pub segment_time: RefCell<String>,
        #[property(get, set)]
        pub best: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SegmentRow {
        const NAME: &'static str = "SegmentRow";
        type Type = super::SegmentRow;
        type ParentType = glib::Object;
    }

    #[glib::derived_properties]
    impl ObjectImpl for SegmentRow {
        fn constructed(&self) {
            self.parent_constructed();
        }
    }
}

glib::wrapper! {
    pub struct SegmentRow(ObjectSubclass<imp::SegmentRow>);
}

impl SegmentRow {
    pub fn new(
        index: u32,
        name: String,
        split_time: String,
        segment_time: String,
        best: String,
    ) -> Self {
        glib::Object::builder()
            .property("index", index)
            .property("name", name)
            .property("split_time", split_time)
            .property("segment_time", segment_time)
            .property("best", best)
            .build()
    }
}
