use crate::unit::Unit;
use gtk::glib::{DateTime, GString, Sender};
use gtk::prelude::*;
use native_dialog::FileDialog;
use relm4::{send, AppUpdate, Components, Model, RelmApp, RelmComponent, WidgetPlus, Widgets};
use relm4_macros::view;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::File;
use std::ops::Not;

mod add_receipt_alert;
mod combobox;
mod schema;
mod unit;

use combobox::AppendAll;

#[derive(Serialize, Deserialize, Debug)]
struct Settings {
    db_file: String,
    capitalize_item_names: bool,
}

#[derive(Debug)]
struct Store {
    name: GString,
    location: GString,
}

#[derive(Debug, Clone)]
struct StoreRow {
    id: i64,
    name: String,
    location: String,
}

#[derive(Debug)]
struct TotalRow {
    unit: String,
    price: i64,
}

impl fmt::Display for TotalRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.price, self.unit)
    }
}

struct Total(Vec<TotalRow>);

impl Total {
    fn new() -> Self {
        Total(Vec::new())
    }

    fn for_receipt(conn: &Connection, receipt_id: i64) -> Self {
        let mut totals_query = conn
            .prepare(
                "SELECT unit, SUM(price * quantity) FROM Item WHERE receipt == ?1 GROUP BY unit;",
            )
            .unwrap();
        let total: Vec<_> = totals_query
            .query_map(params![receipt_id], |row| {
                Ok(TotalRow {
                    unit: row.get(0)?,
                    price: row.get(1)?,
                })
            })
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        Total(total)
    }
}

impl fmt::Display for Total {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.0.is_empty() {
            write!(f, "{}", self.0[0])?;
            for total in &self.0[1..self.0.len()] {
                write!(f, ", {}", total)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
struct Receipt {
    store_idx: Option<u32>,
    date: DateTime,
}

#[derive(Debug)]
struct ReceiptRow {
    id: i64,
    store_name: String,
    date: String,
}

#[derive(Debug)]
struct Item {
    name: GString,
    quantity: u32,
    price: i32,
    unit: Unit,
    receipt_idx: Option<u32>,
}

#[derive(PartialEq, Eq)]
struct InitUpdate {
    page: i32,
    capitalize_item_names: bool,
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum NameStatus {
    Valid,
    NonEmpty,
    Connected,
    Invalid,
}

impl NameStatus {
    fn connect(&mut self) {
        match *self {
            NameStatus::Valid => {}
            NameStatus::NonEmpty => *self = NameStatus::Valid,
            NameStatus::Connected => {}
            NameStatus::Invalid => *self = NameStatus::Connected,
        }
    }

    // we never disconnect from a database, only override a connection if it was successful established

    fn name_non_empty(&mut self) {
        match *self {
            NameStatus::Valid => {}
            NameStatus::NonEmpty => {}
            NameStatus::Connected => *self = NameStatus::Valid,
            NameStatus::Invalid => *self = NameStatus::NonEmpty,
        }
    }

    fn name_empty(&mut self) {
        match *self {
            NameStatus::Valid => *self = NameStatus::Connected,
            NameStatus::NonEmpty => *self = NameStatus::Invalid,
            NameStatus::Connected => {}
            NameStatus::Invalid => {}
        }
    }
}

#[tracker::track]
struct Ui {
    selected_unit: Unit,
    #[tracker::no_eq]
    stores: (Vec<StoreRow>, Option<u32>),
    #[tracker::no_eq]
    receipts: (Vec<ReceiptRow>, Option<u32>),
    #[tracker::do_not_track]
    reset_item_fields: bool,
    #[tracker::do_not_track]
    reset_store_fields: bool,
    #[tracker::no_eq]
    settings_db_path: String,
    #[tracker::no_eq]
    settings_db_path_status: String,
    #[tracker::no_eq]
    settings_db_create_path: String,
    #[tracker::no_eq]
    settings_db_create_path_status: String,
    init_update: InitUpdate,
    store_name_valid: NameStatus,
    store_location_valid: NameStatus,
    item_name_valid: NameStatus,
    #[tracker::no_eq]
    total: Total,
}

struct App {
    conn: Option<Connection>,
    ui: Ui,
}

struct AppComponents {
    dialog: RelmComponent<add_receipt_alert::Dialog, App>,
}

impl Components<App> for AppComponents {
    fn init_components(parent_model: &App, parent_sender: Sender<Msg>) -> Self {
        AppComponents {
            dialog: RelmComponent::new(parent_model, parent_sender),
        }
    }

    fn connect_parent(&mut self, parent_widgets: &AppWidgets) {
        self.dialog.connect_parent(parent_widgets);
    }
}

enum Msg {
    SelectUnit(Unit),
    AddStore(Store),
    AddReceipt(Receipt),
    ForceAddReceipt(i64, GString),
    AddItem(Item),
    OpenDbDialog,
    OpenCreateDbDialog,
    ConnectDb,
    CreateDb,
    Init,
    CapitalizeItem(bool),
    ValidateStoreName(GString),
    ValidateStoreLocation(GString),
    ValidateItemName(GString),
    ReceiptChanged(Option<u32>),
}

impl App {
    fn load_stores(&mut self) {
        if let Some(conn) = &self.conn {
            let mut store_query = conn
                .prepare("SELECT id, name, location FROM Store ORDER BY id ASC;")
                .unwrap();
            let new_stores: Vec<_> = store_query
                .query_map([], |row| {
                    Ok(StoreRow {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        location: row.get(2)?,
                    })
                })
                .unwrap()
                .filter_map(Result::ok)
                .collect();
            let row_to_select = new_stores
                .iter()
                .enumerate()
                .find(|(_, row)| {
                    self.ui
                        .stores
                        .0
                        .binary_search_by_key(&row.id, |old_row| old_row.id)
                        .is_err()
                })
                .map(|rts| rts.0)
                .or_else(|| new_stores.is_empty().not().then_some(new_stores.len() - 1))
                .map(|idx| idx as u32);
            self.ui.set_stores((new_stores, row_to_select));
        }
    }

    fn load_receipts(&mut self) {
        if let Some(conn) = &self.conn {
            let mut store_query = conn.prepare("SELECT Receipt.id, Receipt.date, Store.name FROM Receipt INNER JOIN Store ON Receipt.store = Store.id ORDER BY Receipt.id ASC;").unwrap();
            let new_receipts: Vec<_> = store_query
                .query_map([], |row| {
                    Ok(ReceiptRow {
                        id: row.get(0)?,
                        date: row.get(1)?,
                        store_name: row.get(2)?,
                    })
                })
                .unwrap()
                .filter_map(|row| row.ok())
                .collect();
            let row_to_select = new_receipts
                .iter()
                .enumerate()
                .find(|(_, row)| {
                    self.ui
                        .receipts
                        .0
                        .binary_search_by_key(&row.id, |old_row| old_row.id)
                        .is_err()
                })
                .map(|rts| rts.0)
                .or_else(|| {
                    new_receipts
                        .is_empty()
                        .not()
                        .then_some(new_receipts.len() - 1)
                })
                .map(|idx| idx as u32);
            self.ui.set_receipts((new_receipts, row_to_select));
        }
    }

    fn save_settings(&mut self) {
        if let Ok(file) = File::options()
            .create(true)
            .write(true)
            .truncate(true)
            .open("sqlbon_settings.json")
        {
            let settings = Settings {
                db_file: self.ui.settings_db_path.trim().to_string(),
                capitalize_item_names: self.ui.init_update.capitalize_item_names,
            };
            if serde_json::to_writer(file, &settings).is_ok() {
                self.ui
                    .set_settings_db_path_status("Successfully connected.".to_string());
            } else {
                self.ui.set_settings_db_path_status(
                    "Could not write to sqlbon_settings.json".to_string(),
                );
            }
        } else {
            self.ui
                .set_settings_db_path_status("Could not write to sqlbon_settings.json".to_string());
        }
    }
}

impl AppUpdate for App {
    fn update(
        &mut self,
        msg: Self::Msg,
        components: &Self::Components,
        _sender: Sender<Self::Msg>,
    ) -> bool {
        self.ui.reset();
        self.ui.reset_item_fields = false;
        self.ui.reset_store_fields = false;
        match msg {
            Msg::Init => {
                let mut init_update = InitUpdate {
                    page: 3,
                    capitalize_item_names: false,
                };
                if let Ok(file) = File::open("sqlbon_settings.json") {
                    if let Ok(data) = serde_json::from_reader(file) {
                        let data: Settings = data;
                        if let Ok(conn) = Connection::open(&data.db_file) {
                            self.conn = Some(conn);
                            self.load_stores();
                            self.load_receipts();
                            self.ui.set_settings_db_path(data.db_file);
                            init_update.capitalize_item_names = data.capitalize_item_names;
                            self.ui.update_store_name_valid(NameStatus::connect);
                            self.ui.update_store_location_valid(NameStatus::connect);
                            self.ui.update_item_name_valid(NameStatus::connect);
                            self.ui
                                .set_settings_db_path_status("Successfully connected.".to_string());
                        } else {
                            self.ui.set_settings_db_path_status(format!(
                                "'{}' is not a database file.",
                                data.db_file
                            ));
                        }
                    } else {
                        self.ui.set_settings_db_path_status(
                            "'sqlbon_settings.json' file is not valid.".to_string(),
                        );
                    }
                }
                self.ui.set_init_update(init_update);
            }
            Msg::AddStore(store) => {
                if let Some(conn) = &self.conn {
                    let store_name = store.name.trim();
                    let store_location = store.location.trim();
                    if !store_name.is_empty() && !store_location.is_empty() {
                        let insert_query = conn.execute(
                            "INSERT INTO Store (name, location) VALUES (?1, ?2);",
                            params![store_name, store_location],
                        );
                        if let Err(err) = insert_query {
                            eprintln!("[add store]{err:#?}");
                        } else {
                            self.load_stores();
                            self.ui.reset_store_fields = true;
                        }
                    }
                }
            }
            Msg::AddReceipt(receipt) => {
                if let (Some(conn), Some(store_idx)) = (&self.conn, receipt.store_idx) {
                    let store = &self.ui.stores.0[store_idx as usize];
                    let receipt_date = receipt.date.format("%F").unwrap();
                    let existence_check_query = conn
                        .query_row(
                            "SELECT id FROM Receipt WHERE store == ?1 AND date == ?2;",
                            params![store.id, receipt_date.as_str()],
                            |row| {
                                let id: i64 = row.get(0)?;
                                Ok(id)
                            },
                        )
                        .optional();
                    match existence_check_query {
                        Ok(Some(_)) => {
                            components
                                .dialog
                                .send(add_receipt_alert::DialogMsg::Show(
                                    store.clone(),
                                    receipt.date,
                                ))
                                .unwrap();
                        }
                        Ok(None) => {
                            let insert_query = conn.execute(
                                "INSERT INTO Receipt (store, date) VALUES (?1, ?2);",
                                params![store.id, receipt_date.as_str()],
                            );
                            if let Err(err) = insert_query {
                                eprintln!("[add receipt]{err:#?}");
                            } else {
                                self.load_receipts();
                            }
                        }
                        Err(err) => eprintln!("[add receipt]{err:#?}"),
                    }
                }
            }
            Msg::ForceAddReceipt(store_id, date) => {
                if let Some(conn) = &self.conn {
                    let insert_query = conn.execute(
                        "INSERT INTO Receipt (store, date) VALUES (?1, ?2);",
                        params![store_id, date.as_str()],
                    );
                    if let Err(err) = insert_query {
                        eprintln!("[add receipt]{err:#?}");
                    } else {
                        self.load_receipts();
                    }
                }
            }
            Msg::AddItem(item) => {
                if let (Some(conn), Some(receipt_idx)) = (&self.conn, item.receipt_idx) {
                    let item_name = item.name.trim();
                    if !item_name.is_empty() {
                        let receipt = &self.ui.receipts.0[receipt_idx as usize];
                        let name = if self.ui.init_update.capitalize_item_names {
                            item_name.to_uppercase()
                        } else {
                            item_name.to_string()
                        };
                        let insert_query = conn.execute(
                                "INSERT INTO Item (name, quantity, price, unit, receipt) VALUES (?1, ?2, ?3, ?4, ?5)",
                                params![name, item.quantity, item.price, item.unit.as_str(), receipt.id],
                            );
                        if let Err(err) = insert_query {
                            eprintln!("[add item]{err:#?}");
                        } else {
                            self.ui.reset_item_fields = true;
                        }

                        // update total
                        self.ui.set_total(Total::for_receipt(conn, receipt.id));
                    }
                }
            }
            Msg::SelectUnit(unit) => self.ui.set_selected_unit(unit),
            Msg::OpenDbDialog => {
                let path = FileDialog::new().show_open_single_file().unwrap();
                if let Some(path) = path {
                    let path = path.to_string_lossy().to_string();
                    self.ui.set_settings_db_path(path);
                }
            }
            Msg::OpenCreateDbDialog => {
                let path = FileDialog::new().show_save_single_file().unwrap();
                if let Some(path) = path {
                    let path = path.to_string_lossy().to_string();
                    self.ui.set_settings_db_create_path(path);
                }
            }
            Msg::ConnectDb => {
                if !self.ui.settings_db_path.trim().is_empty() {
                    if let Ok(conn) = Connection::open(self.ui.settings_db_path.trim()) {
                        self.conn = Some(conn);
                        self.load_stores();
                        self.load_receipts();
                        self.save_settings();
                        self.ui.update_store_name_valid(NameStatus::connect);
                        self.ui.update_store_location_valid(NameStatus::connect);
                        self.ui.update_item_name_valid(NameStatus::connect);
                    } else {
                        self.ui.set_settings_db_path_status(
                            "Selected File is not a valid Database.".to_string(),
                        );
                    }
                } else {
                    self.ui
                        .set_settings_db_path_status("No File Selected.".to_string());
                }
            }
            Msg::CreateDb => {
                let db_path = self.ui.settings_db_create_path.trim();
                if !db_path.is_empty() {
                    if File::create(db_path).is_ok() {
                        if let Ok(conn) = Connection::open(db_path) {
                            if conn.execute(schema::SCHEMA_STORE, []).is_ok()
                                && conn.execute(schema::SCHEMA_RECEIPT, []).is_ok()
                                && conn.execute(schema::SCHEMA_ITEM, []).is_ok()
                            {
                                let db_path = db_path.to_string();
                                self.ui.set_settings_db_path(db_path);
                                self.ui.set_settings_db_create_path_status(
                                    "Database created successfully.".to_string(),
                                );
                            } else {
                                let _ = std::fs::remove_file(db_path);
                                self.ui.set_settings_db_create_path_status(
                                    "Could not initialize the database.".to_string(),
                                );
                            }
                        } else {
                            self.ui.set_settings_db_create_path_status(
                                "Could not open the database.".to_string(),
                            );
                        }
                    } else {
                        self.ui.set_settings_db_create_path_status(
                            "Could not create/truncate the file.".to_string(),
                        );
                    }
                } else {
                    self.ui
                        .set_settings_db_create_path_status("No File Selected.".to_string());
                }
            }
            Msg::CapitalizeItem(cap) => {
                self.ui.init_update.capitalize_item_names = cap;
                self.save_settings();
            }
            Msg::ValidateStoreName(name) => {
                if !name.trim().is_empty() {
                    self.ui.update_store_name_valid(NameStatus::name_non_empty);
                } else {
                    self.ui.update_store_name_valid(NameStatus::name_empty);
                }
            }
            Msg::ValidateStoreLocation(location) => {
                if !location.trim().is_empty() {
                    self.ui
                        .update_store_location_valid(NameStatus::name_non_empty);
                } else {
                    self.ui.update_store_location_valid(NameStatus::name_empty);
                }
            }
            Msg::ValidateItemName(name) => {
                if !name.trim().is_empty() {
                    self.ui.update_item_name_valid(NameStatus::name_non_empty);
                } else {
                    self.ui.update_item_name_valid(NameStatus::name_empty);
                }
            }
            Msg::ReceiptChanged(receipt_idx) => {
                if let (Some(conn), Some(receipt_idx)) = (&self.conn, receipt_idx) {
                    let receipt = &self.ui.receipts.0[receipt_idx as usize];
                    self.ui.set_total(Total::for_receipt(conn, receipt.id));
                }
            }
        }
        true
    }
}

#[relm4_macros::widget]
impl Widgets<App, ()> for AppWidgets {
    fn pre_init() {
        view! {
            tab_store = gtk::Label {
                set_label: "Store",
            }
        }
        view! {
            tab_receipt = gtk::Label {
                set_label: "Receipt",
            }
        }
        view! {
            tab_item = gtk::Label {
                set_label: "Item",
            }
        }
        view! {
            tab_settings = gtk::Label {
                set_label: "Settings",
            }
        }
    }

    view! {
        main_window = gtk::ApplicationWindow {
            set_default_width: 1300,
            set_title: Some("SQLBon"),
            set_child: notebook = Some(&gtk::Notebook) {
                set_vexpand: true,
                set_hexpand: true,
                set_valign: gtk::Align::Fill,
                set_halign: gtk::Align::Fill,
                set_page: track!(model.ui.changed(Ui::init_update()), model.ui.init_update.page),

                append_page(Some(&tab_store)) = &gtk::Box {
                    set_vexpand: true,
                    set_hexpand: true,
                    set_valign: gtk::Align::Fill,
                    set_halign: gtk::Align::Fill,
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 5,
                    set_spacing: 5,
                    append = &gtk::Box {
                        set_hexpand: true,
                        set_vexpand: true,
                        set_halign: gtk::Align::Fill,
                        set_valign: gtk::Align::Center,
                        set_orientation: gtk::Orientation::Horizontal,
                        set_margin_all: 5,
                        set_spacing: 5,

                        append = &gtk::Label {
                            set_label: "name:",
                        },
                        append: store_name_entry = &gtk::Entry {
                            set_hexpand: true,
                            set_halign: gtk::Align::Fill,
                            set_text: track!(model.ui.reset_store_fields, ""),
                            connect_changed(sender) => move |store_name| {
                                send!(sender, Msg::ValidateStoreName(store_name.text()));
                            },
                        },
                        append = &gtk::Label {
                            set_label: "location:",
                        },
                        append: location_entry = &gtk::Entry {
                            set_hexpand: true,
                            set_halign: gtk::Align::Fill,
                            set_text: track!(model.ui.reset_store_fields, ""),
                            connect_changed(sender) => move |store_location| {
                                send!(sender, Msg::ValidateStoreLocation(store_location.text()));
                            },
                        },
                    },
                    append = &gtk::Button {
                        set_label: "Add",
                        connect_clicked(sender, store_name_entry, location_entry) => move |_| {
                            send!(sender, Msg::AddStore(Store{
                                name: store_name_entry.text(),
                                location: location_entry.text(),
                            }));
                        },
                        set_sensitive: track!(
                            model.ui.changed(Ui::store_name_valid()) || model.ui.changed(Ui::store_location_valid()),
                            model.ui.store_name_valid == NameStatus::Valid && model.ui.store_location_valid == NameStatus::Valid
                        ),
                    },
                },

                append_page(Some(&tab_receipt)) = &gtk::Box {
                    set_vexpand: true,
                    set_hexpand: true,
                    set_valign: gtk::Align::Fill,
                    set_halign: gtk::Align::Fill,
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 5,
                    set_spacing: 5,
                    append = &gtk::Box {
                        set_hexpand: true,
                        set_vexpand: true,
                        set_halign: gtk::Align::Fill,
                        set_valign: gtk::Align::Center,
                        set_orientation: gtk::Orientation::Horizontal,
                        set_margin_all: 5,
                        set_spacing: 5,

                        append = &gtk::Label {
                            set_label: "store:",
                        },

                        append: store_entry = &gtk::ComboBoxText {
                            set_hexpand: true,
                            set_vexpand: false,
                            set_halign: gtk::Align::Fill,
                            set_valign: gtk::Align::Center,
                            append_all: track!(model.ui.changed(Ui::stores()), model.ui.stores.0.iter().map(|row| format!("{} ({}) #{}", row.name, row.location, row.id)), model.ui.stores.1),
                        },

                        append = &gtk::Label {
                            set_label: "date:",
                        },

                        append: date = &gtk::Calendar {},
                    },
                    append = &gtk::Button {
                        set_label: "Add",
                        connect_clicked(sender, date, store_entry) => move |_| {
                            send!(sender, Msg::AddReceipt(Receipt{
                                store_idx: store_entry.active(),
                                date: date.date(),
                            }));
                        },
                        set_sensitive: watch!(model.conn.is_some()),
                    },
                },
                append_page(Some(&tab_item)) = &gtk::Box {
                    set_vexpand: true,
                    set_hexpand: true,
                    set_valign: gtk::Align::Fill,
                    set_halign: gtk::Align::Fill,
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 5,
                    set_spacing: 5,

                    append = &gtk::Box {
                        set_hexpand: true,
                        set_vexpand: true,
                        set_halign: gtk::Align::Fill,
                        set_valign: gtk::Align::Center,
                        set_orientation: gtk::Orientation::Horizontal,
                        set_margin_all: 5,
                        set_spacing: 5,

                        append = &gtk::Label {
                            set_label: "name:",
                        },
                        append: item_name_entry = &gtk::Entry {
                            set_hexpand: true,
                            set_halign: gtk::Align::Fill,
                            set_text: track!(model.ui.reset_item_fields, ""),
                            connect_changed(sender) => move |item_name| {
                                send!(sender, Msg::ValidateItemName(item_name.text()));
                            },
                        },

                        append = &gtk::Label {
                            set_label: "quantity:",
                        },
                        append: quantity_entry = &gtk::SpinButton {
                            set_hexpand: true,
                            set_halign: gtk::Align::Fill,
                            set_numeric: true,
                            set_digits: 0,
                            set_snap_to_ticks: true,
                            set_range: args!(1.0, 100.0),
                            set_increments: args!(1.0, 5.0),
                            set_value: track!(model.ui.reset_item_fields, 1.0),
                        },

                        append = &gtk::Label {
                            set_label: track!(model.ui.changed(Ui::selected_unit()), &format!("price (×{})", model.ui.selected_unit.scale())),
                        },
                        append: price_entry = &gtk::SpinButton {
                            set_hexpand: true,
                            set_halign: gtk::Align::Fill,
                            set_numeric: true,
                            set_digits: 0,
                            set_range: args!(-1000000.0, 1000000.0),
                            set_increments: args!(10.0, 500.0),
                            set_value: track!(model.ui.reset_item_fields, 1.0),
                        },

                        append = &gtk::Label {
                            set_label: "unit:",
                        },
                        append: unit_entry = &gtk::ComboBoxText {
                            append_all: args!(Unit::ALL.iter().map(|unit| unit.as_str().to_string()), Some(0)),
                            connect_changed(sender) => move |ue| {
                                send!(sender, Msg::SelectUnit(ue.active().unwrap().try_into().unwrap()))
                            }
                        },

                        append = &gtk::Label {
                            set_label: "receipt:",
                        },
                        append: receipt_entry = &gtk::ComboBoxText {
                            append_all: track!(model.ui.changed(Ui::receipts()), model.ui.receipts.0.iter().map(|row| format!("{} ({}) #{}", row.date, row.store_name, row.id)), model.ui.receipts.1),
                            connect_changed(sender) => move |receipt| {
                                send!(sender, Msg::ReceiptChanged(receipt.active()))
                            }
                        },
                    },
                    append = &gtk::Label {
                        set_label: track!(model.ui.changed(Ui::total()), &format!("{}", model.ui.total)),
                    },
                    append = &gtk::Button {
                        set_label: "Add",
                        connect_clicked(sender, item_name_entry, receipt_entry, quantity_entry, unit_entry, price_entry) => move |_| {
                            send!(sender, Msg::AddItem(Item{
                                name: item_name_entry.text(),
                                quantity: quantity_entry.value_as_int() as _,
                                price: price_entry.value_as_int(),
                                unit: unit_entry.active().unwrap().try_into().unwrap(),
                                receipt_idx: receipt_entry.active(),
                            }));
                        },
                        set_sensitive: track!(model.ui.changed(Ui::item_name_valid()), model.ui.item_name_valid == NameStatus::Valid),
                    },
                },
                append_page(Some(&tab_settings)) = &gtk::Grid {
                    set_hexpand: true,
                    set_vexpand: true,
                    set_halign: gtk::Align::Fill,
                    set_valign: gtk::Align::Center,
                    set_orientation: gtk::Orientation::Horizontal,

                    attach(1, 1, 1, 1) = &gtk::Button {
                        set_label: "Connect Database",
                        connect_clicked(sender) => move |_| {
                            send!(sender, Msg::ConnectDb);
                        },
                    },
                    attach(2, 1, 1, 1) = &gtk::Entry {
                        set_hexpand: true,
                        set_text: track!(model.ui.changed(Ui::settings_db_path()), &model.ui.settings_db_path),
                    },
                    attach(3, 1, 1, 1) = &gtk::Button {
                        set_label: "Open File Dialog",
                        connect_clicked(sender) => move |_| {
                            send!(sender, Msg::OpenDbDialog);
                        },
                    },
                    attach(2, 2, 1, 1) = &gtk::Label {
                        set_label: track!(model.ui.changed(Ui::settings_db_path_status()), &model.ui.settings_db_path_status),
                    },
                    attach(1, 3, 1, 1) = &gtk::Button {
                        set_label: "Create Database",
                        connect_clicked(sender) => move |_| {
                            send!(sender, Msg::CreateDb);
                        },
                    },
                    attach(2, 3, 1, 1): settings_create_db_entry = &gtk::Entry {
                        set_hexpand: true,
                        set_text: track!(model.ui.changed(Ui::settings_db_create_path()), &model.ui.settings_db_create_path),
                    },
                    attach(3, 3, 1, 1) = &gtk::Button {
                        set_label: "Open File Dialog",
                        connect_clicked(sender) => move |_| {
                            send!(sender, Msg::OpenCreateDbDialog);
                        },
                    },
                    attach(2, 4, 1, 1) = &gtk::Label {
                        set_label: track!(model.ui.changed(Ui::settings_db_create_path_status()), &model.ui.settings_db_create_path_status),
                    },
                    attach(1, 5, 1, 1) = &gtk::Label {
                        set_label: "Capitalize item names:",
                    },
                    attach(2, 5, 1, 1) = &gtk::CheckButton {
                        set_label: Some("Capitalize"),
                        set_active: track!(model.ui.changed(Ui::init_update()), model.ui.init_update.capitalize_item_names),
                        connect_toggled(sender) => move |cb| {
                            send!(sender, Msg::CapitalizeItem(cb.is_active()))
                        }
                    },
                },
            },
        }
    }

    fn post_init() {
        send!(sender, Msg::Init);
    }
}

impl Model for App {
    type Msg = Msg;
    type Widgets = AppWidgets;
    type Components = AppComponents;
}

fn main() {
    let model = App {
        conn: None,
        ui: Ui {
            selected_unit: Unit::NOK,
            stores: (Vec::new(), None),
            receipts: (Vec::new(), None),
            reset_item_fields: false,
            reset_store_fields: false,
            settings_db_path: String::new(),
            settings_db_path_status: String::new(),
            settings_db_create_path: String::new(),
            settings_db_create_path_status: String::new(),
            init_update: InitUpdate {
                page: 0,
                capitalize_item_names: false,
            },
            store_name_valid: NameStatus::Invalid,
            store_location_valid: NameStatus::Invalid,
            item_name_valid: NameStatus::Invalid,
            total: Total::new(),
            tracker: 0,
        },
    };

    let app = RelmApp::new(model);
    app.run();
}
