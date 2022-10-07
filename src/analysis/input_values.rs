use crate::analysis::{ColumnTypeValue, RowData};
use relm4::factory::{DynamicIndex, FactoryComponent, FactoryComponentSender, FactoryVecDeque};
use relm4::gtk::{self, prelude::*};
use relm4::{ComponentParts, ComponentSender, SimpleComponent};
use std::collections::hash_map::Entry;
use std::collections::HashMap;

#[tracker::track]
#[derive(Debug)]
struct Value {
    #[tracker::no_eq]
    name: String,
    #[tracker::no_eq]
    value: ColumnTypeValue,
}

trait SetDateFromString {
    fn set_date_from_string(&self, date: &ColumnTypeValue);
}

impl SetDateFromString for gtk::Calendar {
    fn set_date_from_string(&self, date: &ColumnTypeValue) {
        if let ColumnTypeValue::Date(date) = date {
            let mut chunks = date.split('-');
            let year: i32 = chunks.next().unwrap().parse().unwrap();
            let month: i32 = chunks.next().unwrap().parse().unwrap();
            let day: i32 = chunks.next().unwrap().parse().unwrap();
            self.set_year(year);
            self.set_month(month - 1);
            self.set_day(day);
        }
    }
}

#[relm4::factory]
impl FactoryComponent for Value {
    type CommandOutput = ();
    type Init = (String, ColumnTypeValue);
    type Input = ColumnTypeValue;
    type Output = ();
    type ParentInput = InputValueMsg;
    type ParentWidget = gtk::Box;
    type Widgets = Valuewidgets;

    view! {
        #[name(date_selector)]
        gtk::Popover {
            gtk::Calendar {
                #[track(self.changed(Value::value()))]
                set_date_from_string: &self.value,
                connect_day_selected[sender, date_button] => move |this| {
                    let date = this.date().format("%F").unwrap();
                    date_button.set_label(&date);
                    sender.input(ColumnTypeValue::Date(date.to_string()));
                },
            }
        },
        #[root]
        #[name(root_box)]
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            gtk::Label {
                #[track]
                set_text: &self.name,
            },
            append: t = match &self.value {
                ColumnTypeValue::String(s) => {
                    gtk::Entry {
                        #[track(self.changed(Value::value()))]
                        set_text: s,
                        set_size_request: (150, -1),
                        set_margin_end: 2,
                        set_margin_start: 2,
                        connect_changed[sender] => move |this| {
                            sender.input(ColumnTypeValue::String(this.text().trim().to_string()));
                        },
                    }
                },
                ColumnTypeValue::Number(n) => {
                    gtk::SpinButton {
                        set_numeric: true,
                        set_digits: 0,
                        set_snap_to_ticks: true,
                        set_increments: (1.0, 10.0),
                        set_range: (0.0, f64::MAX),
                        #[track(self.changed(Value::value()))]
                        set_value: *n as f64,
                        set_size_request: (150, -1),
                        set_margin_end: 2,
                        set_margin_start: 2,
                        connect_changed[sender] => move |this| {
                            sender.input(ColumnTypeValue::Number(this.value() as i64));
                        },
                    }
                }
                ColumnTypeValue::Date(d) => {
                    #[name(date_button)]
                    gtk::MenuButton {
                        #[track(self.changed(Value::value()))]
                        set_label: d,
                        set_popover: Some(&date_selector),
                        set_size_request: (150, -1),
                        set_margin_end: 2,
                        set_margin_start: 2,
                    }
                }
            }
        }
    }

    fn init_model(
        (name, value): Self::Init,
        _index: &DynamicIndex,
        _sender: FactoryComponentSender<Self>,
    ) -> Self {
        Value {
            name,
            value,
            tracker: Value::value() | Value::name(),
        }
    }

    fn update(&mut self, message: Self::Input, _sender: FactoryComponentSender<Self>) {
        self.reset();
        self.value = message;
    }
}

pub(crate) struct InputValue {
    data: HashMap<String, Vec<ColumnTypeValue>>,
    values: FactoryVecDeque<Value>,
    show: String,
}

#[derive(Debug)]
pub(crate) enum InputValueMsg {
    Replicate(String, RowData),
}

#[relm4::component(pub(crate))]
impl SimpleComponent for InputValue {
    type Input = InputValueMsg;
    type Output = ();
    type Init = ();
    type Widgets = InputValueWidgets;

    view! {
        #[root]
        #[name(values)]
        gtk::Box {
           set_orientation: gtk::Orientation::Vertical,
        }
    }

    fn init(
        _init: Self::Init,
        root: &Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let widgets = view_output!();

        let model = InputValue {
            data: HashMap::new(),
            show: String::new(),
            values: FactoryVecDeque::new(widgets.values.clone(), &sender.input),
        };

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        self.update(message, sender);
    }
}

impl InputValue {
    fn update(&mut self, message: InputValueMsg, _sender: ComponentSender<Self>) {
        match message {
            InputValueMsg::Replicate(name, mut row_data) => {
                let mut v = self.values.guard();
                // ------ save current data -----------
                let old_name = std::mem::replace(&mut self.show, name.clone());
                self.data.insert(
                    old_name,
                    v.iter().map(|row_entry| row_entry.value.clone()).collect(),
                );

                // -------- load old data -------------
                match self.data.entry(name) {
                    Entry::Vacant(e) => {
                        v.clear();
                        row_data.0.sort_by_key(|row_entry| row_entry.id);

                        let values = row_data
                            .0
                            .into_iter()
                            .map(|row_entry| {
                                let v_ty: ColumnTypeValue = row_entry.ty.into();
                                v.push_back((row_entry.name, v_ty.clone()));
                                v_ty
                            })
                            .collect();
                        e.insert(values);
                    }
                    Entry::Occupied(mut o) => {
                        row_data.0.sort_by_key(|row_entry| row_entry.id);

                        let current_len = v.len();
                        let row_len = row_data.0.len();
                        let old_data = o.get();
                        let values = row_data
                            .0
                            .into_iter()
                            .enumerate()
                            .map(|(i, row_entry)| {
                                let v_ty = old_data
                                    .get(row_entry.id)
                                    .and_then(|value| {
                                        value.is_column_type(row_entry.ty).then(|| value.clone())
                                    })
                                    .unwrap_or_else(|| row_entry.ty.into());

                                if i < current_len {
                                    if let Some(value) = v.get_mut(i) {
                                        value.set_name(row_entry.name);
                                        value.set_value(v_ty.clone());
                                    };
                                } else {
                                    v.push_back((row_entry.name, v_ty.clone()));
                                }
                                v_ty
                            })
                            .collect();
                        if current_len > row_len {
                            for _ in row_len..current_len {
                                v.pop_back();
                            }
                        }
                        o.insert(values);
                    }
                }
            }
        }
    }

    pub fn get_input_values(&self) -> Vec<(String, ColumnTypeValue)> {
        self.values
            .iter()
            .map(|row| (row.name.clone(), row.value.clone()))
            .collect()
    }
}
