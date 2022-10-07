use relm4::gtk::{self, prelude::*};

pub(crate) trait AppendDialog {
    fn append(&self, widget: &impl IsA<gtk::Widget>);
}

impl AppendDialog for gtk::Dialog {
    fn append(&self, widget: &impl IsA<gtk::Widget>) {
        self.content_area().append(widget);
    }
}
