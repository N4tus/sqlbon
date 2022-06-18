use gtk::prelude::ComboBoxExtManual;

pub trait AppendAll {
    fn append_all(&self, data: impl IntoIterator<Item = String>, to_select: Option<u32>);
}

impl AppendAll for gtk::ComboBoxText {
    fn append_all(&self, data: impl IntoIterator<Item = String>, to_select: Option<u32>) {
        self.remove_all();
        for d in data {
            self.append(None, &d);
        }
        println!("{to_select:?}");
        self.set_active(to_select);
    }
}
