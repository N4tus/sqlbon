use crate::analysis::edit_query_dialog::QueryDialog;
// use crate::analysis::type_component::{TypeParent, Validity};
use crate::combobox::AppendAll;
use crate::Msg;
use itertools::Itertools;
use relm4::gtk;
use relm4::gtk::glib::{GString, Type, Value};
use relm4::gtk::prelude::*;
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller, SimpleComponent,
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::convert::identity;
use std::error::Error;
use std::fs::File;
use std::rc::Rc;
use tap::TapFallible;

mod edit_query_dialog;
mod type_def;

#[derive(Debug)]
pub(crate) enum AnalysisMsg {
    PopulateModel(usize),
    NewQuery(String),
    EditQuery(usize),
    DeleteQuery(usize),
    EditQueryResult(Query, String, usize),
    ConnectDb(Rc<Connection>),
    QuerySelected(Option<usize>),
    NewQueryNameChanged(GString),
    // ValidityChanged(Validity),
}

#[tracker::track]
pub(crate) struct Analysis {
    #[tracker::no_eq]
    analysis: Option<Data>,
    #[tracker::no_eq]
    queries: Vec<(String, Query)>,
    #[tracker::do_not_track]
    conn: Option<Rc<Connection>>,
    #[tracker::do_not_track]
    new_button_valid: bool,
    selected_query: Option<usize>,
    query_selected: bool,
    #[tracker::do_not_track]
    query_dialog: Controller<edit_query_dialog::QueryDialog>,
    // #[tracker::do_not_track]
    // type_component: Controller<type_component::TypeModel>,
}

struct Data {
    store: gtk::ListStore,
    query_id: usize,
}

// impl TypeParent for Analysis {
//     fn validity_changed(validity: Validity) -> Self::Msg {
//         AnalysisMsg::ValidityChanged(validity)
//     }
// }

#[relm4::component(pub(crate))]
impl SimpleComponent for Analysis {
    type Input = AnalysisMsg;
    type Output = Msg;
    type Init = gtk::Window;
    type Widgets = AnalysisWidgets;

    view! {
        #[root]
        #[name(analysis_box)]
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            gtk::Grid {
                set_vexpand: true,
                set_valign: gtk::Align::Center,
                attach[0, 0, 2, 1]: selected_query = &gtk::ComboBoxText {
                    #[track(model.changed(Analysis::queries()))]
                    append_all: model.queries.iter().map(|(n, _)|n).cloned(),
                    #[track(model.changed(Analysis::selected_query()))]
                    set_active: model.selected_query.map(|id| id.try_into().unwrap()),
                    connect_changed[sender] => move |query| {
                        sender.input(AnalysisMsg::QuerySelected(query.active().map(|id| id as usize)));
                    },
                },
                attach[0, 1, 1, 1]: name_entry = &gtk::Entry {
                    connect_changed[sender] => move |name| {
                        sender.input(AnalysisMsg::NewQueryNameChanged(name.text()));
                    },
                },
                attach[1, 1, 1, 1] = &gtk::Button {
                    set_label: "new",
                    #[watch]
                    set_sensitive: model.new_button_valid,
                    connect_clicked[sender, name_entry] => move |_| {
                        let name = name_entry.text();
                        let name = name.trim();
                        if !name.is_empty() {
                            name_entry.set_text("");
                            sender.input(AnalysisMsg::NewQuery(name.to_string()));
                        }
                    },
                },
                attach[0, 2, 1, 1] = &gtk::Button {
                    set_label: "edit",
                    #[track(model.changed(Analysis::query_selected()))]
                    set_sensitive: model.query_selected,
                    connect_clicked[sender, selected_query] => move |_| {
                        if let Some(id) = selected_query.active() {
                            sender.input(AnalysisMsg::EditQuery(id as usize));
                        }
                    },
                },
                attach[1, 2, 1, 1] = &gtk::Button {
                    set_label: "delete",
                    #[track(model.changed(Analysis::query_selected()))]
                    set_sensitive: model.query_selected,
                    connect_clicked[sender, selected_query] => move |_| {
                        if let Some(id) = selected_query.active() {
                            sender.input(AnalysisMsg::DeleteQuery(id as usize));
                        }
                    },
                },
                attach[0, 3, 2, 1] = &gtk::Button {
                    set_label: "execute",
                    #[track(model.changed(Analysis::query_selected()))]
                    set_sensitive: model.query_selected,
                    connect_clicked[sender, selected_query] => move |_| {
                        if let Some(id) = selected_query.active() {
                            sender.input(AnalysisMsg::PopulateModel(id as usize));
                        }
                    },
                },
                // attach[0, 4, 2, 1] = &gtk::Separator {},
                // attach[0, 4, 2, 1]: components.type_component.root_widget(),
            },
            gtk::ScrolledWindow {
                #[name(list)]
                gtk::TreeView {
                    set_hexpand: true,
                    set_vexpand: false,
                    set_valign: gtk::Align::Center,
                },
            },
        }
    }

    fn post_view() {
        let model: &Analysis = model;
        if model.changed(Analysis::analysis()) {
            if let Some(data) = &model.analysis {
                if let Some((_, q)) = model.queries.get(data.query_id) {
                    for (i, (q, _)) in q.table_header.0.iter().enumerate() {
                        let i: i32 = i.try_into().unwrap();
                        if let Some(column) = list.column(i) {
                            column.set_title(q);
                        } else {
                            let cell = gtk::CellRendererText::new();
                            let column = gtk::TreeViewColumn::new();
                            column.set_title(q);
                            column.pack_start(&cell, true);
                            column.set_attributes(&cell, &[("text", i)]);
                            column.set_sort_column_id(i);
                            column.set_resizable(true);

                            list.append_column(&column);
                        }
                    }
                    let i: i32 = q.table_header.0.len().try_into().unwrap();
                    while let Some(column) = list.column(i) {
                        list.remove_column(&column);
                    }
                    list.set_model(Some(&data.store));
                }
            }
        }
    }

    fn init(
        parent_window: Self::Init,
        root: &Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let query_dialog = QueryDialog::builder()
            .launch(parent_window)
            .forward(sender.input_sender(), identity);

        let model = Analysis {
            analysis: None,
            queries: read_queries()
                .tap_err(|err| println!("[read queries]{err:#?}"))
                .ok()
                .unwrap_or_default(),
            conn: None,
            new_button_valid: false,
            selected_query: None,
            query_selected: false,
            query_dialog,
            tracker: 0,
        };

        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        self.reset();
        match message {
            AnalysisMsg::PopulateModel(id) => {
                if let (Some(conn), Some((_, query))) = (&self.conn, self.queries.get(id)) {
                    match Analysis::exec_query(conn, id, query) {
                        Ok(data) => self.set_analysis(Some(data)),
                        Err(err) => eprintln!("[exec query]{err:#?}"),
                    }
                }
            }
            AnalysisMsg::ConnectDb(db) => self.conn = Some(db),
            AnalysisMsg::EditQueryResult(query, name, id) => {
                // no track update, because name should already be in the map
                self.update_queries(|q| {
                    if let Some((n, q)) = q.get_mut(id) {
                        *q = query;
                        *n = name;
                    }
                });
                // force change
                self.update_selected_query(|sq| *sq = Some(id));
                save_queries(&self.queries).unwrap();
            }
            AnalysisMsg::NewQuery(name) => {
                if !self.queries.iter().map(|(n, _)| n).any(|n| n == &name) {
                    self.update_queries(move |q| {
                        q.push((name, Query::new()));
                    });
                    let id = self.queries.len() - 1;
                    self.set_selected_query(Some(id));
                    self.query_dialog
                        .emit(edit_query_dialog::QueryDialogMsg::Open {
                            query: Query::new(),
                            id,
                            names: self.queries.iter().map(|(n, _)| n).cloned().collect(),
                            ok_button_name: "add".to_string(),
                        });
                }
            }
            AnalysisMsg::EditQuery(id) => {
                let q = &self.queries[id];
                self.query_dialog
                    .emit(edit_query_dialog::QueryDialogMsg::Open {
                        query: q.1.clone(),
                        id,
                        names: self.queries.iter().map(|(n, _)| n).cloned().collect(),
                        ok_button_name: "edit".to_string(),
                    });
            }
            AnalysisMsg::DeleteQuery(name) => {
                self.update_queries(|q| {
                    q.remove(name);
                });
                save_queries(&self.queries).unwrap();
            }
            AnalysisMsg::QuerySelected(active) => {
                self.selected_query = active;
                self.set_query_selected(active.is_some());
            }
            AnalysisMsg::NewQueryNameChanged(name) => {
                let name = name.trim();
                self.new_button_valid =
                    !name.is_empty() && !self.queries.iter().map(|(n, _)| n).any(|n| n == name);
            } // AnalysisMsg::ValidityChanged(val) => println!("validity: {val:?}"),
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub(crate) enum ColumnType {
    String,
    Number,
    Date,
}

enum ColumnTypeValue {
    String(String),
    Number(i64),
}

impl ToString for ColumnType {
    fn to_string(&self) -> String {
        match self {
            ColumnType::String => "String".to_string(),
            ColumnType::Number => "Number".to_string(),
            ColumnType::Date => "Date".to_string(),
        }
    }
}

impl ToValue for ColumnTypeValue {
    fn to_value(&self) -> Value {
        match self {
            ColumnTypeValue::String(s) => s.to_value(),
            ColumnTypeValue::Number(n) => n.to_value(),
        }
    }

    fn value_type(&self) -> Type {
        match self {
            ColumnTypeValue::String(s) => s.value_type(),
            ColumnTypeValue::Number(n) => n.value_type(),
        }
    }
}

impl From<ColumnType> for Type {
    fn from(ct: ColumnType) -> Self {
        match ct {
            ColumnType::String => Type::STRING,
            ColumnType::Number => Type::I64,
            ColumnType::Date => Type::STRING,
        }
    }
}

#[derive(Debug)]
pub(crate) struct NumberOutOfRange(u32);

impl TryFrom<u32> for ColumnType {
    type Error = NumberOutOfRange;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ColumnType::String),
            1 => Ok(ColumnType::Number),
            2 => Ok(ColumnType::Date),
            other => Err(NumberOutOfRange(other)),
        }
    }
}

impl From<ColumnType> for u32 {
    fn from(ct: ColumnType) -> Self {
        match ct {
            ColumnType::String => 0,
            ColumnType::Number => 1,
            ColumnType::Date => 2,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct RowData(pub(crate) Vec<(String, ColumnType)>);

impl RowData {
    pub(crate) fn new() -> Self {
        RowData(Vec::new())
    }

    pub(crate) fn is_filled(&self) -> bool {
        self.0.iter().all(|row| !row.0.is_empty())
    }

    pub(crate) fn has_entries(&self) -> bool {
        !self.0.is_empty()
    }

    pub(crate) fn all_names_unique(&self) -> bool {
        self.0.iter().unique_by(|(n, _)| n).count() == self.0.len()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Query {
    sql: String,
    table_header: RowData,
}

impl Query {
    fn new() -> Self {
        Query {
            sql: String::new(),
            table_header: RowData::new(),
        }
    }
}

impl Analysis {
    fn exec_query(
        conn: &Connection,
        query_id: usize,
        query: &Query,
    ) -> Result<Data, Box<dyn Error>> {
        let sql_query = conn.prepare(&query.sql);
        match sql_query {
            Ok(mut stmt) => {
                let ctypes: Vec<Type> = query
                    .table_header
                    .0
                    .iter()
                    .map(|&(_, ty)| ty.into())
                    .collect();

                let store = gtk::ListStore::new(ctypes.as_slice());
                let mut rows = stmt.query([]).unwrap();
                while let Some(row) = rows.next()? {
                    let mut values = Vec::with_capacity(query.table_header.0.len());
                    for (i, (_, cty)) in query.table_header.0.iter().enumerate() {
                        match *cty {
                            ColumnType::String | ColumnType::Date => {
                                let v: String = row.get(i)?;
                                values.push(ColumnTypeValue::String(v));
                            }
                            ColumnType::Number => {
                                let v: i64 = row.get(i)?;
                                values.push(ColumnTypeValue::Number(v));
                            }
                        }
                    }
                    let mut value_refs = Vec::with_capacity(query.table_header.0.len());
                    for (i, value) in values.iter().enumerate() {
                        value_refs.push((i as u32, value as &dyn ToValue));
                    }

                    let iter = store.append();
                    store.set(&iter, value_refs.as_slice());
                }
                Ok(Data { store, query_id })
            }
            Err(err) => {
                eprintln!("[populate model]{err:#?}");
                Err(Box::new(err))
            }
        }
    }
}

fn save_queries(queries: &[(String, Query)]) -> std::io::Result<()> {
    if let Ok(file) = File::options()
        .create(true)
        .write(true)
        .truncate(true)
        .open("./sqlbon_queries.json")
    {
        serde_json::to_writer(file, queries)?;
    }
    Ok(())
}

fn read_queries() -> std::io::Result<Vec<(String, Query)>> {
    let file = File::open("./sqlbon_queries.json")?;
    let data = serde_json::from_reader(file)?;
    Ok(data)
}
