use crate::analysis::edit_query_dialog::QueryDialogMsg;
use crate::combobox::AppendAll;
use crate::{App, Msg};
use gtk::glib::{GString, Type, Value};
use gtk::prelude::*;
use gtk::ScrollablePolicy;
use relm4::{send, ComponentUpdate, Components, Model, RelmComponent, Sender, Widgets};
use relm4_macros::view;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs::File;
use std::rc::Rc;
use tap::TapFallible;

mod edit_query_dialog;
mod type_def;

pub(crate) enum AnalysisMsg {
    PopulateModel(usize),
    NewQuery(String),
    EditQuery(usize),
    DeleteQuery(usize),
    EditQueryResult(Query, String, usize),
    ConnectDb(Rc<Connection>),
    QuerySelected(Option<usize>),
    NewQueryNameChanged(GString),
}

#[tracker::track]
pub(crate) struct AnalysisModel {
    #[tracker::do_not_track]
    analysis: Analysis,
    #[tracker::no_eq]
    queries: Vec<(String, Query)>,
    #[tracker::do_not_track]
    conn: Option<Rc<Connection>>,
    #[tracker::do_not_track]
    new_button_valid: bool,
    selected_query: Option<usize>,
    query_selected: bool,
}

#[tracker::track]
struct Analysis {
    #[tracker::no_eq]
    model: Option<Data>,
}

struct Data {
    store: gtk::ListStore,
    query_id: usize,
}

pub(crate) struct AnalysisComponents {
    query_dialog: RelmComponent<edit_query_dialog::QueryDialogModel, AnalysisModel>,
}

impl Components<AnalysisModel> for AnalysisComponents {
    fn init_components(parent_model: &AnalysisModel, parent_sender: Sender<AnalysisMsg>) -> Self {
        AnalysisComponents {
            query_dialog: RelmComponent::new(parent_model, parent_sender),
        }
    }

    fn connect_parent(&mut self, parent_widgets: &AnalysisWidgets) {
        self.query_dialog.connect_parent(parent_widgets);
    }
}

impl Model for AnalysisModel {
    type Msg = AnalysisMsg;
    type Widgets = AnalysisWidgets;
    type Components = AnalysisComponents;
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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
pub(crate) struct Query {
    sql: String,
    table_header: Vec<(String, ColumnType)>,
}

impl Query {
    fn new() -> Self {
        Query {
            sql: String::new(),
            table_header: Vec::new(),
        }
    }
}

impl Analysis {
    fn exec_query(
        &mut self,
        conn: &Connection,
        query_id: usize,
        query: &Query,
    ) -> Result<(), Box<dyn Error>> {
        let sql_query = conn.prepare(&query.sql);
        match sql_query {
            Ok(mut stmt) => {
                let ctypes: Vec<Type> = query
                    .table_header
                    .iter()
                    .map(|&(_, ty)| ty.into())
                    .collect();

                let store = gtk::ListStore::new(ctypes.as_slice());
                let mut rows = stmt.query([]).unwrap();
                while let Some(row) = rows.next()? {
                    let mut values = Vec::with_capacity(query.table_header.len());
                    for (i, (_, cty)) in query.table_header.iter().enumerate() {
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
                    let mut value_refs = Vec::with_capacity(query.table_header.len());
                    for (i, value) in values.iter().enumerate() {
                        value_refs.push((i as u32, value as &dyn ToValue));
                    }

                    let iter = store.append();
                    store.set(&iter, value_refs.as_slice());
                }
                self.set_model(Some(Data { store, query_id }));
            }
            Err(err) => eprintln!("[populate model]{err:#?}"),
        }
        Ok(())
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

impl ComponentUpdate<App> for AnalysisModel {
    fn init_model(_parent_model: &App) -> Self {
        AnalysisModel {
            analysis: Analysis {
                model: None,
                tracker: 0,
            },
            queries: read_queries()
                .tap_err(|err| println!("[read queries]{err:#?}"))
                .ok()
                .unwrap_or_default(),
            conn: None,
            new_button_valid: false,
            selected_query: None,
            query_selected: false,
            tracker: 0,
        }
    }

    fn update(
        &mut self,
        msg: AnalysisMsg,
        components: &AnalysisComponents,
        _sender: Sender<AnalysisMsg>,
        _parent_sender: Sender<Msg>,
    ) {
        self.analysis.reset();
        self.reset();
        match msg {
            AnalysisMsg::PopulateModel(id) => {
                if let (Some(conn), Some((_, query))) = (&self.conn, self.queries.get(id)) {
                    if let Err(err) = self.analysis.exec_query(conn, id, query) {
                        eprintln!("[exec query]{err:#?}");
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
                    send!(
                        components.query_dialog,
                        QueryDialogMsg::Open {
                            query: Query::new(),
                            id,
                            names: self.queries.iter().map(|(n, _)| n).cloned().collect(),
                            ok_button_name: "add".to_string(),
                        }
                    )
                }
            }
            AnalysisMsg::EditQuery(id) => {
                let q = &self.queries[id];
                send!(
                    components.query_dialog,
                    QueryDialogMsg::Open {
                        query: q.1.clone(),
                        id,
                        names: self.queries.iter().map(|(n, _)| n).cloned().collect(),
                        ok_button_name: "edit".to_string(),
                    }
                )
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
            }
        }
    }
}

#[relm4_macros::widget(pub(crate))]
impl Widgets<AnalysisModel, App> for AnalysisWidgets {
    view! {
        analysis_box = gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            append = &gtk::Grid {
                set_vexpand: true,
                set_valign: gtk::Align::Center,
                attach(0, 0, 2, 1): selected_query = &gtk::ComboBoxText {
                    append_all: track!(model.changed(AnalysisModel::queries()), model.queries.iter().map(|(n, _)|n).cloned(), None),
                    set_active: track!(model.changed(AnalysisModel::selected_query()), model.selected_query.map(|id| id.try_into().unwrap())),
                    connect_changed(sender) => move |query| {
                        send!(sender, AnalysisMsg::QuerySelected(query.active().map(|id| id as usize)));
                    },
                },
                attach(0, 1, 1, 1): name_entry = &gtk::Entry {
                    connect_changed(sender) => move |name| {
                        send!(sender, AnalysisMsg::NewQueryNameChanged(name.text()));
                    },
                },
                attach(1, 1, 1, 1) = &gtk::Button {
                    set_label: "new",
                    set_sensitive: watch!(model.new_button_valid),
                    connect_clicked(sender, name_entry) => move |_| {
                        let name = name_entry.text();
                        let name = name.trim();
                        if !name.is_empty() {
                            name_entry.set_text("");
                            send!(sender, AnalysisMsg::NewQuery(name.to_string()));
                        }
                    },
                },
                attach(0, 2, 1, 1) = &gtk::Button {
                    set_label: "edit",
                    set_sensitive: track!(model.changed(AnalysisModel::query_selected()), model.query_selected),
                    connect_clicked(sender, selected_query) => move |_| {
                        if let Some(id) = selected_query.active() {
                            send!(sender, AnalysisMsg::EditQuery(id as usize));
                        }
                    },
                },
                attach(1, 2, 1, 1) = &gtk::Button {
                    set_label: "delete",
                    set_sensitive: track!(model.changed(AnalysisModel::query_selected()), model.query_selected),
                    connect_clicked(sender, selected_query) => move |_| {
                        if let Some(id) = selected_query.active() {
                            send!(sender, AnalysisMsg::DeleteQuery(id as usize));
                        }
                    },
                },
                attach(0, 3, 2, 1) = &gtk::Button {
                    set_label: "execute",
                    set_sensitive: track!(model.changed(AnalysisModel::query_selected()), model.query_selected),
                    connect_clicked(sender, selected_query) => move |_| {
                        if let Some(id) = selected_query.active() {
                            send!(sender, AnalysisMsg::PopulateModel(id as usize));
                        }
                    },
                },
            },
            append = &gtk::ScrolledWindow {
                set_child: list = Some(&gtk::TreeView) {
                    set_hexpand: true,
                    set_vexpand: false,
                    set_valign: gtk::Align::Center,
                    set_vscroll_policy: ScrollablePolicy::Natural,
                },
            },
        }
    }

    fn pre_init() {
        let main_window = None;
    }

    fn post_view() {
        let model: &AnalysisModel = model;
        if model.analysis.changed(Analysis::model()) {
            if let Some(data) = &model.analysis.model {
                let data: &Data = data;
                if let Some((_, q)) = model.queries.get(data.query_id) {
                    for (i, (q, _)) in q.table_header.iter().enumerate() {
                        let i: i32 = i.try_into().unwrap();
                        if let Some(column) = list.column(i) {
                            column.set_title(q);
                        } else {
                            let cell = gtk::CellRendererText::new();
                            view! {
                                column = gtk::TreeViewColumn {
                                    set_title: q,
                                    pack_start: args!(&cell, true),
                                    set_attributes: args!(&cell, &[("text", i)]),
                                    set_sort_column_id: i,
                                    set_resizable: true,
                                }
                            }
                            list.append_column(&column);
                        }
                    }
                    let i: i32 = q.table_header.len().try_into().unwrap();
                    while let Some(column) = list.column(i) {
                        list.remove_column(&column);
                    }
                    list.set_model(Some(&data.store));
                }
            }
        }
    }

    additional_fields! {
        main_window: Option<gtk::ApplicationWindow>,
    }

    fn post_connect_parent(&mut self, parent_widgets: &AppWidgets) {
        self.main_window = Some(parent_widgets.main_window.clone());
    }
}
