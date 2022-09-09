use gtk::prelude::ComboBoxExtManual;
use relm4::gtk;

pub trait AppendAll {
    fn append_all_and_select(&self, data: impl IntoIterator<Item = String>, to_select: Option<u32>);
    fn append_all(&self, data: impl IntoIterator<Item = String>);
}

impl AppendAll for gtk::ComboBoxText {
    fn append_all_and_select(
        &self,
        data: impl IntoIterator<Item = String>,
        to_select: Option<u32>,
    ) {
        self.remove_all();
        for d in data {
            self.append(None, &d);
        }
        self.set_active(to_select);
    }

    fn append_all(&self, data: impl IntoIterator<Item = String>) {
        self.remove_all();
        for d in data {
            self.append(None, &d);
        }
    }
}
