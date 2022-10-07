use crate::analysis::edit_query_dialog::QueryDialog;
use crate::analysis::input_values::{InputValue, InputValueMsg};
use crate::combobox::AppendAll;
use crate::Msg;
use relm4::gtk;
use relm4::gtk::glib::{DateTime, GString, Type, Value};
use relm4::gtk::prelude::*;
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller, SimpleComponent,
};
use rusqlite::types::ToSqlOutput;
use rusqlite::{Connection, ToSql};
use serde::{Deserialize, Serialize};
use std::convert::identity;
use std::fmt::Formatter;
use std::fs::File;
use std::rc::Rc;
use tap::TapFallible;

mod edit_query_dialog;
mod input_values;
mod type_component;

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
    #[tracker::do_not_track]
    input_values: Controller<input_values::InputValue>,
    #[tracker::no_eq]
    query_error: String,
}

struct Data {
    store: gtk::ListStore,
    query_id: usize,
}

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
                    #[track]
                    set_sensitive: model.query_selected,
                    connect_clicked[sender, selected_query] => move |_| {
                        if let Some(id) = selected_query.active() {
                            sender.input(AnalysisMsg::EditQuery(id as usize));
                        }
                    },
                },
                attach[1, 2, 1, 1] = &gtk::Button {
                    set_label: "delete",
                    #[track]
                    set_sensitive: model.query_selected,
                    connect_clicked[sender, selected_query] => move |_| {
                        if let Some(id) = selected_query.active() {
                            sender.input(AnalysisMsg::DeleteQuery(id as usize));
                        }
                    },
                },
                attach[0, 3, 2, 1] = &gtk::Button {
                    set_label: "execute",
                    #[track]
                    set_sensitive: model.query_selected,
                    connect_clicked[sender, selected_query] => move |_| {
                        if let Some(id) = selected_query.active() {
                            sender.input(AnalysisMsg::PopulateModel(id as usize));
                        }
                    },
                },
            },
            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_vexpand: true,
                gtk::ScrolledWindow {
                    set_vexpand: true,
                    #[name(list)]
                    gtk::TreeView {
                        set_hexpand: true,
                        set_vexpand: true,
                    },
                },
                gtk::Label {
                    #[track]
                    set_text: &model.query_error,
                    set_vexpand: false,
                }
            },
            gtk::ScrolledWindow {
                set_child: Some(model.input_values.widget()),
                set_hexpand: false,
                set_vexpand: true,
                set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),
            },
        }
    }

    fn post_view() {
        let model: &Analysis = model;
        if model.changed(Analysis::analysis()) {
            if let Some(data) = &model.analysis {
                if let Some((_, q)) = model.queries.get(data.query_id) {
                    for (i, row_entry) in q.table_header.0.iter().enumerate() {
                        let i: i32 = i.try_into().unwrap();
                        if let Some(column) = list.column(i) {
                            column.set_title(&row_entry.name);
                        } else {
                            let cell = gtk::CellRendererText::new();
                            let column = gtk::TreeViewColumn::new();
                            column.set_title(&row_entry.name);
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

        let input_values = InputValue::builder().launch(()).detach();

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
            input_values,
            query_error: String::new(),
            tracker: 0,
        };

        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        self.update(message, sender);
    }
}

impl Analysis {
    fn update(&mut self, message: AnalysisMsg, _sender: ComponentSender<Self>) {
        self.reset();
        match message {
            AnalysisMsg::PopulateModel(id) => {
                if let (Some(conn), Some((_, query))) = (&self.conn, self.queries.get(id)) {
                    let values = self.input_values.state().get().model.get_input_values();

                    match Analysis::exec_query(conn, id, query, values) {
                        Ok(data) => {
                            self.set_analysis(Some(data));
                            self.set_query_error(String::new());
                        }
                        Err(err_str) => {
                            self.set_query_error(err_str);
                        }
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

                if let Some(active) = active {
                    if let Some((name, q)) = self.queries.get(active) {
                        self.input_values.emit(InputValueMsg::Replicate(
                            name.clone(),
                            q.query_input.clone(),
                        ));
                    }
                }
            }
            AnalysisMsg::NewQueryNameChanged(name) => {
                let name = name.trim();
                self.new_button_valid =
                    !name.is_empty() && !self.queries.iter().map(|(n, _)| n).any(|n| n == name);
            }
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub(crate) enum ColumnType {
    String,
    Number,
    Date,
}

impl std::fmt::Display for ColumnType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match *self {
            ColumnType::String => f.write_str("String"),
            ColumnType::Number => f.write_str("Number"),
            ColumnType::Date => f.write_str("Date"),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum ColumnTypeValue {
    String(String),
    Number(i64),
    Date(String),
}

impl ToSql for ColumnTypeValue {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(match self {
            ColumnTypeValue::String(s) => {
                ToSqlOutput::Borrowed(rusqlite::types::ValueRef::Text(s.as_bytes()))
            }
            ColumnTypeValue::Number(n) => ToSqlOutput::Owned(rusqlite::types::Value::Integer(*n)),
            ColumnTypeValue::Date(d) => {
                ToSqlOutput::Borrowed(rusqlite::types::ValueRef::Text(d.as_bytes()))
            }
        })
    }
}

impl ColumnTypeValue {
    fn is_column_type(&self, ty: ColumnType) -> bool {
        match self {
            ColumnTypeValue::String(_) => ty == ColumnType::String,
            ColumnTypeValue::Number(_) => ty == ColumnType::Number,
            ColumnTypeValue::Date(_) => ty == ColumnType::Date,
        }
    }
}

impl ToValue for ColumnTypeValue {
    fn to_value(&self) -> Value {
        match self {
            ColumnTypeValue::String(s) => s.to_value(),
            ColumnTypeValue::Number(n) => n.to_value(),
            ColumnTypeValue::Date(d) => d.to_value(),
        }
    }

    fn value_type(&self) -> Type {
        match self {
            ColumnTypeValue::String(s) => s.value_type(),
            ColumnTypeValue::Number(n) => n.value_type(),
            ColumnTypeValue::Date(d) => d.value_type(),
        }
    }
}

impl From<ColumnType> for u32 {
    fn from(ty: ColumnType) -> Self {
        match ty {
            ColumnType::String => 0,
            ColumnType::Number => 1,
            ColumnType::Date => 2,
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

impl From<ColumnType> for ColumnTypeValue {
    fn from(ty: ColumnType) -> Self {
        match ty {
            ColumnType::String => ColumnTypeValue::String(String::new()),
            ColumnType::Number => ColumnTypeValue::Number(0),
            ColumnType::Date => ColumnTypeValue::Date(
                DateTime::now_local()
                    .unwrap()
                    .format("%F")
                    .unwrap()
                    .to_string(),
            ),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct RowEntry {
    name: String,
    ty: ColumnType,
    #[serde(skip)]
    id: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct RowData(pub(crate) Vec<RowEntry>);

impl RowData {
    pub(crate) fn new() -> Self {
        RowData(Vec::new())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Query {
    sql: String,
    table_header: RowData,
    query_input: RowData,
}

impl Query {
    fn new() -> Self {
        Query {
            sql: String::new(),
            table_header: RowData::new(),
            query_input: RowData::new(),
        }
    }
}

impl From<ExecQueryErrConv> for String {
    fn from(err: ExecQueryErrConv) -> Self {
        let conversion_failure = |column_idx: usize,
                                  expected_type: rusqlite::types::Type,
                                  sql_column_name: String| {
            let column_name = err.failed_name;
            let given_type = err.given_type;
            let expected_type = if expected_type == rusqlite::types::Type::Integer {
                "Number"
            } else {
                "String/Text"
            };
            format!("Mismatch between the output type '{column_name}' (at {column_idx}) and the query column type for '{sql_column_name}': column type: `{given_type}` <-> query type `{expected_type}`")
        };
        match err.err {
            rusqlite::Error::FromSqlConversionFailure(column_idx, expected_type, _) => {
                conversion_failure(column_idx, expected_type, "<unknown>".to_string())
            }
            rusqlite::Error::InvalidColumnType(column_idx, sql_column_name, expected_type) => {
                conversion_failure(column_idx, expected_type, sql_column_name)
            }
            rusqlite::Error::InvalidParameterName(param) => {
                format!("The query did not expected to receive a parameter '{param}'.")
            }
            rusqlite::Error::InvalidColumnIndex(_) => {
                "The query has less columns than the amount of given output types.".to_string()
            }
            rusqlite::Error::InvalidQuery => "The Query is invalid.".to_string(),
            rusqlite::Error::MultipleStatement => {
                "The query contains multiple statements. Only one is allowed".to_string()
            }
            err => {
                eprintln!("[execute query]{err:#?}");
                "Unknown error".to_string()
            }
        }
    }
}

struct ExecQueryErrConv {
    err: rusqlite::Error,
    given_type: ColumnType,
    failed_name: String,
}

impl ExecQueryErrConv {
    fn new(
        given_type: ColumnType,
        failed_name: &str,
    ) -> impl FnOnce(rusqlite::Error) -> ExecQueryErrConv + '_ {
        move |err| ExecQueryErrConv {
            err,
            given_type,
            failed_name: failed_name.to_string(),
        }
    }
    fn empty() -> impl FnOnce(rusqlite::Error) -> ExecQueryErrConv {
        |err| ExecQueryErrConv {
            err,
            given_type: ColumnType::String,
            failed_name: String::new(),
        }
    }
}

impl Analysis {
    fn exec_query(
        conn: &Connection,
        query_id: usize,
        query: &Query,
        mut input_data: Vec<(String, ColumnTypeValue)>,
    ) -> Result<Data, String> {
        let mut stmt = conn
            .prepare(&query.sql)
            .map_err(ExecQueryErrConv::empty())?;
        let ctypes: Vec<Type> = query
            .table_header
            .0
            .iter()
            .map(|row_entry| row_entry.ty.into())
            .collect();

        let store = gtk::ListStore::new(ctypes.as_slice());

        for (n, _) in &mut input_data {
            n.insert(0, ':');
        }
        let input_data: Vec<_> = input_data
            .iter()
            .map(|(n, v)| (n.as_str(), v as &dyn ToSql))
            .collect();
        let mut rows = stmt
            .query(input_data.as_slice())
            .map_err(ExecQueryErrConv::empty())?;
        while let Some(row) = rows.next().map_err(ExecQueryErrConv::empty())? {
            let mut values = Vec::with_capacity(query.table_header.0.len());
            for (i, row_entry) in query.table_header.0.iter().enumerate() {
                match row_entry.ty {
                    ColumnType::String => {
                        let v: String = row
                            .get(i)
                            .map_err(ExecQueryErrConv::new(ColumnType::String, &row_entry.name))?;
                        values.push(ColumnTypeValue::String(v));
                    }
                    ColumnType::Number => {
                        let v: i64 = row
                            .get(i)
                            .map_err(ExecQueryErrConv::new(ColumnType::Number, &row_entry.name))?;
                        values.push(ColumnTypeValue::Number(v));
                    }
                    ColumnType::Date => {
                        let v: String = row
                            .get(i)
                            .map_err(ExecQueryErrConv::new(ColumnType::Date, &row_entry.name))?;
                        values.push(ColumnTypeValue::Date(v));
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
    let mut data: Vec<(String, Query)> = serde_json::from_reader(file)?;
    for (_, q) in &mut data {
        let mut id_counter = 0;
        for row in &mut q.table_header.0 {
            row.id = id_counter;
            id_counter += 1;
        }
        id_counter = 0;
        for row in &mut q.query_input.0 {
            row.id = id_counter;
            id_counter += 1;
        }
    }
    Ok(data)
}
