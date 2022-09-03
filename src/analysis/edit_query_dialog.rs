use crate::analysis::{AnalysisModel, ColumnType, Query};
use crate::{AnalysisMsg, AppendAll};
use gtk::prelude::*;
use relm4::{send, ComponentUpdate, Model, Sender, WidgetPlus, Widgets};
use std::cell::Cell;
use std::rc::Rc;

pub(crate) struct QueryDialogModel {
    hidden: bool,
    op: HeaderOperation,
    indices: Vec<Rc<Cell<u32>>>,
    name: String,
}

pub(crate) enum QueryDialogMsg {
    Open(Query, String),
    Accept(Query),
    Cancel,
    HeaderOp(HeaderOperation),
}

pub(crate) enum HeaderOperation {
    Delete(Rc<Cell<u32>>),
    Add(Rc<Cell<u32>>),
    MoveUp(Rc<Cell<u32>>),
    MoveDown(Rc<Cell<u32>>),
    Replicate(Query, u32),
    None,
}

impl Model for QueryDialogModel {
    type Msg = QueryDialogMsg;
    type Widgets = QueryDialogWidgets;
    type Components = ();
}

impl ComponentUpdate<AnalysisModel> for QueryDialogModel {
    fn init_model(_parent_model: &AnalysisModel) -> Self {
        QueryDialogModel {
            hidden: true,
            op: HeaderOperation::None,
            indices: vec![Rc::new(Cell::new(0))],
            name: "".to_string(),
        }
    }

    fn update(
        &mut self,
        msg: QueryDialogMsg,
        _components: &(),
        _sender: Sender<QueryDialogMsg>,
        parent_sender: Sender<AnalysisMsg>,
    ) {
        self.op = HeaderOperation::None;
        match msg {
            QueryDialogMsg::Open(query, name) => {
                self.hidden = false;
                self.name = name;
                let len = self.indices.len();
                for _ in 0..(len - 1) {
                    self.indices.remove(0);
                }
                self.indices[0].set(query.table_header.len() as u32);
                for idx in (0..query.table_header.len() as u32).rev() {
                    self.indices.insert(0, Rc::new(Cell::new(idx)));
                }
                self.op = HeaderOperation::Replicate(query, len as u32 - 1);
            }
            QueryDialogMsg::Accept(query) => {
                self.hidden = true;
                send!(
                    parent_sender,
                    AnalysisMsg::EditQueryResult(query, self.name.clone())
                );
            }
            QueryDialogMsg::Cancel => {
                self.hidden = true;
            }
            QueryDialogMsg::HeaderOp(op) => match op {
                HeaderOperation::Add(idx) => {
                    let idx_value = idx.get();

                    for index in &self.indices[idx_value as usize..] {
                        index.set(index.get() + 1);
                    }

                    let idx = Rc::new(Cell::new(idx_value));
                    self.indices.insert(idx_value as usize, Rc::clone(&idx));
                    self.op = HeaderOperation::Add(idx);
                }
                HeaderOperation::Delete(idx) => {
                    let idx_value = idx.get();

                    for index in &self.indices[(idx_value as usize + 1)..] {
                        index.set(index.get() - 1);
                    }

                    let idx = Rc::new(Cell::new(idx_value));
                    self.indices.insert(idx.get() as usize, Rc::clone(&idx));
                    self.op = HeaderOperation::Delete(idx);
                }
                HeaderOperation::MoveUp(idx) => {
                    if idx.get() > 0 {
                        self.op = HeaderOperation::MoveUp(idx);
                    }
                }
                HeaderOperation::MoveDown(idx) => {
                    if (idx.get() as usize + 1) < self.indices.len() {
                        self.op = HeaderOperation::MoveDown(idx);
                    }
                }
                HeaderOperation::None | HeaderOperation::Replicate(_, _) => {
                    self.op = HeaderOperation::None
                }
            },
        }
    }
}

trait SetContent {
    fn append_content(&self, widget: &impl IsA<gtk::Widget>);
}

impl SetContent for gtk::Dialog {
    fn append_content(&self, widget: &impl IsA<gtk::Widget>) {
        self.content_area().append(widget);
    }
}

#[relm4_macros::widget(pub(crate))]
impl Widgets<QueryDialogModel, AnalysisModel> for QueryDialogWidgets {
    fn pre_init() {
        let new_button_idx = Rc::clone(&model.indices[0]);
    }
    view! {
        dialog = gtk::Dialog {
            set_modal: true,
            set_default_width: 1000,
            set_visible: watch!(!model.hidden),

            append_content: sql_entry = &gtk::Entry {
                set_hexpand: true,
                set_halign: gtk::Align::Fill,
            },
            append_content: headers = &gtk::Grid {
                set_hexpand: true,
                set_vexpand: true,
                set_halign: gtk::Align::Fill,
                set_valign: gtk::Align::Center,
                set_orientation: gtk::Orientation::Horizontal,
                attach(2, 0, 1, 1) = &gtk::Button {
                    set_label: "new",
                    connect_clicked(sender, new_button_idx) => move |_| {
                        send!(sender, QueryDialogMsg::HeaderOp(HeaderOperation::Add(Rc::clone(&new_button_idx))));
                    },
                },
            },

            add_button: args!("Add", gtk::ResponseType::Accept),
            add_button: args!("Cancel", gtk::ResponseType::Cancel),
            connect_response(sender, sql_entry, headers, new_button_idx) => move |_, resp| {
                send!(sender, if resp == gtk::ResponseType::Accept {
                    let count_types = new_button_idx.get();
                    if count_types == 0 {
                        QueryDialogMsg::Cancel
                    } else {
                        let table_header: Vec<_> = (0..count_types as i32).map(|ty_idx| {
                            let name = headers
                                .child_at(0, ty_idx)
                                .unwrap()
                                .downcast::<gtk::Entry>()
                                .unwrap()
                                .text()
                                .trim()
                                .to_string();
                            let ty = headers
                                .child_at(1, ty_idx)
                                .unwrap()
                                .downcast::<gtk::ComboBoxText>()
                                .unwrap()
                                .active()
                                .map(ColumnType::try_from)
                                .unwrap()
                                .unwrap();
                            (name, ty)
                        }).collect();
                        QueryDialogMsg::Accept(Query {
                            sql: sql_entry.text().trim().to_string(),
                            table_header,
                        })
                    }
                } else {
                    QueryDialogMsg::Cancel
                });
            }
        }
    }

    fn post_init() {
        let content: gtk::Box = dialog.content_area();
        content.set_hexpand(true);
        content.set_vexpand(true);
        content.set_halign(gtk::Align::Fill);
        content.set_valign(gtk::Align::Center);
        content.set_orientation(gtk::Orientation::Vertical);
        content.set_margin_all(5);
    }

    fn post_view() {
        let model: &QueryDialogModel = model;
        let headers: &gtk::Grid = headers;
        fn add_row(
            index: i32,
            idx: &Rc<Cell<u32>>,
            headers: &gtk::Grid,
            sender: &Sender<QueryDialogMsg>,
            init: Option<(&str, ColumnType)>,
        ) {
            let name = gtk::Entry::new();
            let ty = gtk::ComboBoxText::new();
            ty.append_all(
                [
                    ColumnType::String.to_string(),
                    ColumnType::Number.to_string(),
                    ColumnType::Date.to_string(),
                ],
                Some(0),
            );
            if let Some((n, t)) = init {
                name.set_text(n);
                ty.set_active(Some(t.into()));
            }
            let new_row = gtk::Button::with_label("new");
            let delete_row = gtk::Button::with_label("delete");
            let move_row_up = gtk::Button::with_label("up");
            let move_row_down = gtk::Button::with_label("down");
            {
                let sender = sender.clone();
                let idx = Rc::clone(idx);
                new_row.connect_clicked(move |_| {
                    send!(
                        sender,
                        QueryDialogMsg::HeaderOp(HeaderOperation::Add(Rc::clone(&idx)))
                    )
                });
            }
            {
                let sender = sender.clone();
                let idx = Rc::clone(idx);
                delete_row.connect_clicked(move |_| {
                    send!(
                        sender,
                        QueryDialogMsg::HeaderOp(HeaderOperation::Delete(Rc::clone(&idx)))
                    )
                });
            }
            {
                let sender = sender.clone();
                let idx = Rc::clone(idx);
                move_row_up.connect_clicked(move |_| {
                    send!(
                        sender,
                        QueryDialogMsg::HeaderOp(HeaderOperation::MoveUp(Rc::clone(&idx)))
                    )
                });
            }
            {
                let sender = sender.clone();
                let idx = Rc::clone(idx);
                move_row_down.connect_clicked(move |_| {
                    send!(
                        sender,
                        QueryDialogMsg::HeaderOp(HeaderOperation::MoveDown(Rc::clone(&idx)))
                    )
                });
            }
            headers.insert_row(index);
            headers.attach(&name, 0, index, 1, 1);
            headers.attach(&ty, 1, index, 1, 1);
            headers.attach(&new_row, 2, index, 1, 1);
            headers.attach(&delete_row, 3, index, 1, 1);
            headers.attach(&move_row_up, 4, index, 1, 1);
            headers.attach(&move_row_down, 5, index, 1, 1);
        }
        match &model.op {
            HeaderOperation::Add(idx) => {
                let index: i32 = idx.get().try_into().unwrap();
                add_row(index, idx, headers, &sender, None);
            }
            HeaderOperation::Delete(idx) => {
                let index: i32 = idx.get().try_into().unwrap();
                headers.remove_row(index);
            }
            HeaderOperation::MoveUp(idx) => {
                let index: i32 = idx.get().try_into().unwrap();
                if index > 0 {
                    let current = headers
                        .child_at(0, index)
                        .unwrap()
                        .downcast::<gtk::Entry>()
                        .unwrap();
                    let upper = headers
                        .child_at(0, index - 1)
                        .unwrap()
                        .downcast::<gtk::Entry>()
                        .unwrap();
                    let tmp = current.text();
                    current.set_text(&upper.text());
                    upper.set_text(&tmp);
                    let current = headers
                        .child_at(1, index)
                        .unwrap()
                        .downcast::<gtk::ComboBoxText>()
                        .unwrap();
                    let upper = headers
                        .child_at(1, index - 1)
                        .unwrap()
                        .downcast::<gtk::ComboBoxText>()
                        .unwrap();
                    let tmp = current.active();
                    current.set_active(upper.active());
                    upper.set_active(tmp);
                }
            }
            HeaderOperation::MoveDown(idx) => {
                let index: i32 = idx.get().try_into().unwrap();
                // the last row is the lonely new button, which we also need to avoid.
                //                   v
                if (index as usize + 2) < model.indices.len() {
                    let current = headers
                        .child_at(0, index)
                        .unwrap()
                        .downcast::<gtk::Entry>()
                        .unwrap();
                    let lower = headers
                        .child_at(0, index + 1)
                        .unwrap()
                        .downcast::<gtk::Entry>()
                        .unwrap();
                    let tmp = current.text();
                    current.set_text(&lower.text());
                    lower.set_text(&tmp);
                    let current = headers
                        .child_at(1, index)
                        .unwrap()
                        .downcast::<gtk::ComboBoxText>()
                        .unwrap();
                    let lower = headers
                        .child_at(1, index + 1)
                        .unwrap()
                        .downcast::<gtk::ComboBoxText>()
                        .unwrap();
                    let tmp = current.active();
                    current.set_active(lower.active());
                    lower.set_active(tmp);
                }
            }
            HeaderOperation::Replicate(query, len) => {
                for _ in 0..*len {
                    headers.remove_row(0);
                }
                let sql_entry: &gtk::Entry = sql_entry;
                sql_entry.set_text(&query.sql);
                for (index, (name, ty)) in query.table_header.iter().enumerate() {
                    add_row(
                        index.try_into().unwrap(),
                        &Rc::new(Cell::new(index.try_into().unwrap())),
                        headers,
                        &sender,
                        Some((name.as_str(), *ty)),
                    );
                }
            }
            HeaderOperation::None => {}
        }
    }

    fn post_connect_parent(&mut self, parent_widgets: &AnalysisWidgets) {
        self.dialog
            .set_transient_for(parent_widgets.main_window.as_ref());
    }
}
