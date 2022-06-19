use crate::unit::Unit;
use gtk::glib::{DateTime, GString, Sender};
use gtk::prelude::*;
use gtk::Align;
use native_dialog::FileDialog;
use relm4::{send, AppUpdate, Model, RelmApp, WidgetPlus, Widgets};
use relm4_macros::view;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::ops::Not;

mod combobox;
mod schema;
mod unit;

use combobox::AppendAll;

#[derive(Serialize, Deserialize, Debug)]
struct Settings {
    db_file: String,
}

#[derive(Debug)]
struct Store {
    name: GString,
    location: GString,
}

#[derive(Debug)]
struct StoreRow {
    id: i64,
    name: String,
    location: String,
}

#[derive(Debug)]
struct Receipt {
    store_idx: u32,
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
    price: u32,
    unit: Unit,
    receipt_idx: u32,
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
}

struct App {
    conn: Option<Connection>,
    ui: Ui,
}

enum Msg {
    SelectUnit(Unit),
    AddStore(Store),
    AddReceipt(Receipt),
    AddItem(Item),
    OpenDbDialog,
    OpenCreateDbDialog,
    ConnectDb,
    CreateDb,
    Init,
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
                .filter_map(|row| row.ok())
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
                .or_else(|| new_stores.is_empty().not().then(|| new_stores.len() - 1))
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
                        .then(|| new_receipts.len() - 1)
                })
                .map(|idx| idx as u32);
            self.ui.set_receipts((new_receipts, row_to_select));
        }
    }
}

impl AppUpdate for App {
    fn update(
        &mut self,
        msg: Self::Msg,
        _components: &Self::Components,
        _sender: Sender<Self::Msg>,
    ) -> bool {
        self.ui.reset();
        self.ui.reset_item_fields = false;
        self.ui.reset_store_fields = false;
        match msg {
            Msg::Init => {
                if let Ok(file) = File::open("sqlbon_settings.json") {
                    if let Ok(data) = serde_json::from_reader(file) {
                        let data: Settings = data;
                        if let Ok(conn) = Connection::open(&data.db_file) {
                            self.conn = Some(conn);
                            self.load_stores();
                            self.load_receipts();
                            self.ui.set_settings_db_path(data.db_file);
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
            }
            Msg::AddStore(store) => {
                if let Some(conn) = &self.conn {
                    let insert_query = conn.execute(
                        "INSERT INTO Store (name, location) VALUES (?1, ?2);",
                        params![store.name.as_str(), store.location.as_str()],
                    );
                    if let Err(err) = insert_query {
                        eprintln!("[add store]{err:#?}");
                    } else {
                        self.load_stores();
                        self.ui.reset_store_fields = true;
                    }
                }
            }
            Msg::AddReceipt(receipts) => {
                if let Some(conn) = &self.conn {
                    let store = &self.ui.stores.0[receipts.store_idx as usize];
                    let insert_query = conn.execute(
                        "INSERT INTO Receipt (store, date) VALUES (?1, ?2);",
                        params![store.id, receipts.date.format("%F").unwrap().as_str()],
                    );
                    if let Err(err) = insert_query {
                        eprintln!("[add receipt]{err:#?}");
                    } else {
                        self.load_receipts();
                    }
                }
            }
            Msg::AddItem(item) => {
                if let Some(conn) = &self.conn {
                    let receipt = &self.ui.receipts.0[item.receipt_idx as usize];
                    let insert_query = conn.execute(
                        "INSERT INTO Item (name, quantity, price, unit, receipt) VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![item.name.as_str(), item.quantity, item.price, item.unit.as_str(), receipt.id],
                    );
                    if let Err(err) = insert_query {
                        eprintln!("[add item]{err:#?}");
                    } else {
                        self.ui.reset_item_fields = true;
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
                        if let Ok(file) = File::options()
                            .create(true)
                            .write(true)
                            .truncate(true)
                            .open("sqlbon_settings.json")
                        {
                            let settings = Settings {
                                db_file: self.ui.settings_db_path.trim().to_string(),
                            };
                            if serde_json::to_writer(file, &settings).is_ok() {
                                self.ui.set_settings_db_path_status(
                                    "Successfully connected.".to_string(),
                                );
                            } else {
                                self.ui.set_settings_db_path_status(
                                    "Could not write to sqlbon_settings.json".to_string(),
                                );
                            }
                        } else {
                            self.ui.set_settings_db_path_status(
                                "Could not write to sqlbon_settings.json".to_string(),
                            );
                        }
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
        gtk::ApplicationWindow {
            set_default_width: 1300,
            set_title: Some("SQLBon"),
            set_child: notebook = Some(&gtk::Notebook) {
                set_vexpand: true,
                set_hexpand: true,
                set_valign: Align::Fill,
                set_halign: Align::Fill,

                append_page(Some(&tab_store)) = &gtk::Box {
                    set_vexpand: true,
                    set_hexpand: true,
                    set_valign: Align::Fill,
                    set_halign: Align::Fill,
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 5,
                    set_spacing: 5,
                    append = &gtk::Box {
                        set_hexpand: true,
                        set_vexpand: true,
                        set_halign: Align::Fill,
                        set_valign: Align::Center,
                        set_orientation: gtk::Orientation::Horizontal,
                        set_margin_all: 5,
                        set_spacing: 5,

                        append = &gtk::Label {
                            set_label: "name:",
                        },
                        append: store_name_entry = &gtk::Entry {
                            set_hexpand: true,
                            set_halign: Align::Fill,
                            set_text: track!(model.ui.reset_store_fields, ""),
                        },
                        append = &gtk::Label {
                            set_label: "location:",
                        },
                        append: location_entry = &gtk::Entry {
                            set_hexpand: true,
                            set_halign: Align::Fill,
                            set_text: track!(model.ui.reset_store_fields, ""),
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
                        set_sensitive: watch!(model.conn.is_some()),
                    },
                },

                append_page(Some(&tab_receipt)) = &gtk::Box {
                    set_vexpand: true,
                    set_hexpand: true,
                    set_valign: Align::Fill,
                    set_halign: Align::Fill,
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 5,
                    set_spacing: 5,
                    append = &gtk::Box {
                        set_hexpand: true,
                        set_vexpand: true,
                        set_halign: Align::Fill,
                        set_valign: Align::Center,
                        set_orientation: gtk::Orientation::Horizontal,
                        set_margin_all: 5,
                        set_spacing: 5,

                        append = &gtk::Label {
                            set_label: "store:",
                        },

                        append: store_entry = &gtk::ComboBoxText {
                            set_hexpand: true,
                            set_vexpand: false,
                            set_halign: Align::Fill,
                            set_valign: Align::Center,
                            append_all: track!(model.ui.changed(Ui::stores()), model.ui.stores.0.iter().map(|row| format!("{} ({})", row.name, row.location)), model.ui.stores.1),
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
                                store_idx: store_entry.active().unwrap(),
                                date: date.date(),
                            }));
                        },
                        set_sensitive: watch!(model.conn.is_some()),
                    },
                },
                append_page(Some(&tab_item)) = &gtk::Box {
                    set_vexpand: true,
                    set_hexpand: true,
                    set_valign: Align::Fill,
                    set_halign: Align::Fill,
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 5,
                    set_spacing: 5,

                    append = &gtk::Box {
                        set_hexpand: true,
                        set_vexpand: true,
                        set_halign: Align::Fill,
                        set_valign: Align::Center,
                        set_orientation: gtk::Orientation::Horizontal,
                        set_margin_all: 5,
                        set_spacing: 5,

                        append = &gtk::Label {
                            set_label: "name:",
                        },
                        append: item_name_entry = &gtk::Entry {
                            set_hexpand: true,
                            set_halign: Align::Fill,
                            set_text: track!(model.ui.reset_item_fields, ""),
                        },

                        append = &gtk::Label {
                            set_label: "quantity:",
                        },
                        append: quantity_entry = &gtk::SpinButton {
                            set_hexpand: true,
                            set_halign: Align::Fill,
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
                            set_halign: Align::Fill,
                            set_numeric: true,
                            set_digits: 0,
                            set_range: args!(1.0, 1000000.0),
                            set_increments: args!(10.0, 500.0),
                            set_value: track!(model.ui.reset_item_fields, 1.0),
                        },

                        append = &gtk::Label {
                            set_label: "unit:",
                        },
                        append: unit_entry = &gtk::ComboBoxText {
                            connect_changed(sender) => move |ue| {
                                send!(sender, Msg::SelectUnit(ue.active().unwrap().try_into().unwrap()))
                            }
                        },

                        append = &gtk::Label {
                            set_label: "receipt:",
                        },
                        append: receipt_entry = &gtk::ComboBoxText {
                            append_all: track!(model.ui.changed(Ui::receipts()), model.ui.receipts.0.iter().map(|row| format!("{} ({})", row.date, row.store_name)), model.ui.receipts.1),
                        },
                    },
                    append = &gtk::Button {
                        set_label: "Add",
                        connect_clicked(sender, item_name_entry, receipt_entry, quantity_entry, unit_entry, price_entry) => move |_| {
                            send!(sender, Msg::AddItem(Item{
                                name: item_name_entry.text(),
                                quantity: quantity_entry.value_as_int() as _,
                                price: price_entry.value_as_int() as _,
                                unit: unit_entry.active().unwrap().try_into().unwrap(),
                                receipt_idx: receipt_entry.active().unwrap(),
                            }));
                        },
                        set_sensitive: watch!(model.conn.is_some()),
                    },
                },
                append_page(Some(&tab_settings)) = &gtk::Grid {
                    set_hexpand: true,
                    set_vexpand: true,
                    set_halign: Align::Fill,
                    set_valign: Align::Center,
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
                }
            },
        }
    }

    fn post_init() {
        {
            let unit_entry: &gtk::ComboBoxText = &unit_entry;
            for unit in Unit::ALL {
                unit_entry.append(None, unit.into());
            }
            unit_entry.set_active(Some(0));
            send!(sender, Msg::Init);
        }
    }
}

impl Model for App {
    type Msg = Msg;
    type Widgets = AppWidgets;
    type Components = ();
}

fn main() {
    std::fs::copy(
        "/home/janek/Downloads/sqlite-tools-linux-x86-3360000/expenses",
        "/home/janek/Downloads/sqlite-tools-linux-x86-3360000/expenses-test",
    )
    .unwrap();

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
            tracker: 0,
        },
    };

    let app = RelmApp::new(model);
    app.run();
}
