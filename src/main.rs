use gtk::glib::{DateTime, GString, Sender, Type};
use gtk::prelude::*;
use gtk::{ListStore, SensitivityType, Widget};
use relm4::{AppUpdate, Model, RelmApp, send, Widgets, WidgetPlus};
use relm4_macros::view;
use rusqlite::{Connection, params};
use rusqlite::types::Value;
use crate::unit::Unit;

mod unit;

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
    store: i64,
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

struct App {
    conn: Connection,
    stores: Vec<StoreRow>,
    receipts: Vec<ReceiptRow>,
}

enum Msg {
    AddStore(Store),
    AddReceipt(Receipt),
    AddItem(Item),
}

impl AppUpdate for App {
    fn update(&mut self, msg: Self::Msg, components: &Self::Components, sender: Sender<Self::Msg>) -> bool {
        match msg {
            Msg::AddStore(store) => {
                let insert_query = self.conn.execute("INSERT INTO Store (name, location) VALUES (?1, ?2);", params![store.name.as_str(), store.location.as_str()]);
                if let Err(err) = insert_query {
                    eprintln!("[add store]{err:#?}");
                }
            },
            Msg::AddReceipt(receipts) => {
                let store = &self.stores[receipts.store_idx as usize];
                let insert_query = self.conn.execute("INSERT INTO Receipt (store, date) VALUES (?1, ?2);", params![store.id, receipts.date.format("%F").unwrap().as_str()]);
                if let Err(err) = insert_query {
                    eprintln!("[add receipt]{err:#?}");
                }
            },
            Msg::AddItem(item) =>{
                println!("{item:#?}");
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
    }

    view! {
        gtk::ApplicationWindow {
            set_title: Some("SQLBon"),
            set_default_width: 300,
            set_default_height: 100,
            set_child = Some(&gtk::Notebook) {
                append_page(Some(&tab_store)) = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 5,
                    set_spacing: 5,
                    append = &gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_margin_all: 5,
                        set_spacing: 5,

                        append = &gtk::Label {
                            set_label: "name:",
                        },

                        append: store_name_entry = &gtk::Entry {},

                        append = &gtk::Label {
                            set_label: "location:",
                        },

                        append: location_entry = &gtk::Entry {},
                    },

                    append = &gtk::Button {
                        set_label: "Add",
                        connect_clicked(sender, store_name_entry, location_entry) => move |_| {
                            send!(sender, Msg::AddStore(Store{
                                name: store_name_entry.text(),
                                location: location_entry.text(),
                            }));
                        },
                    },
                },

                append_page(Some(&tab_receipt)) = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 5,
                    set_spacing: 5,
                    append = &gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_margin_all: 5,
                        set_spacing: 5,

                        append = &gtk::Label {
                            set_label: "store:",
                        },

                        append: store_entry = &gtk::ComboBoxText { },

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
                    },
                },
                append_page(Some(&tab_item)) = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 5,
                    set_spacing: 5,
                    append = &gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_margin_all: 5,
                        set_spacing: 5,

                        append = &gtk::Label {
                            set_label: "name:",
                        },
                        append: item_name_entry = &gtk::Entry { },

                        append = &gtk::Label {
                            set_label: "quantity:",
                        },
                        append: quantity_entry = &gtk::SpinButton {
                            set_numeric: true,
                            set_digits: 0,
                            set_snap_to_ticks: true,
                            set_range: args!(1.0, 100.0),
                            set_increments: args!(1.0, 5.0),
                        },

                        append = &gtk::Label {
                            set_label: "price:",
                        },
                        append = &gtk::Label {
                            set_label: "unit:",
                        },
                        append: unit_entry = &gtk::ComboBoxText { },

                        append = &gtk::Label {
                            set_label: "receipt:",
                        },
                        append: receipt_entry = &gtk::ComboBoxText { },
                    },
                    append = &gtk::Button {
                        set_label: "Add",
                        connect_clicked(sender, item_name_entry, receipt_entry, quantity_entry, unit_entry) => move |_| {
                            send!(sender, Msg::AddItem(Item{
                                name: item_name_entry.text(),
                                quantity: quantity_entry.value_as_int() as _,
                                price: 100,
                                unit: unit_entry.active().unwrap().try_into().unwrap(),
                                receipt_idx: receipt_entry.active().unwrap(),
                            }));
                        },
                    },
                },
            }
        }
    }

    fn post_init() {
        {
            let store_entry: &gtk::ComboBoxText = &store_entry;
            for row in &model.stores {
                let t = format!("{} ({})", row.name, row.location);
                store_entry.append(None, &t);
            }
            store_entry.set_active(Some(0));
        }
        {
            let receipt_entry: &gtk::ComboBoxText = &receipt_entry;
            let receipts: &[ReceiptRow] = &model.receipts;
            for row in receipts {
                let t = format!("{} ({})", row.date, row.store_name);
                receipt_entry.append(None, &t);
            }
            receipt_entry.set_active(Some(0));
        }
        {
            let unit_entry: &gtk::ComboBoxText = &unit_entry;
            for unit in Unit::ALL {
                unit_entry.append(None, unit.into());
            }
            unit_entry.set_active(Some(0));
        }
    }
}

impl Model for App {
    type Msg = Msg;
    type Widgets = AppWidgets;
    type Components = ();
}

fn main() {
    std::fs::copy("/home/janek/Downloads/sqlite-tools-linux-x86-3360000/expenses", "/home/janek/Downloads/sqlite-tools-linux-x86-3360000/expenses-test").unwrap();
    let conn = Connection::open("/home/janek/Downloads/sqlite-tools-linux-x86-3360000/expenses-test").unwrap();

    let stores = {
        let mut store_query = conn.prepare("SELECT id, name, location FROM Store;").unwrap();
        store_query.query_map([], |row|
            Ok(StoreRow{
                id: row.get(0)?,
                name: row.get(1)?,
                location: row.get(2)?,
            })
        ).unwrap().filter_map(|row| row.ok()).collect()
    };

    let receipts = {
        let mut store_query = conn.prepare("SELECT Receipt.id, Receipt.store, Receipt.date, Store.name FROM Receipt INNER JOIN Store ON Receipt.store = Store.id ORDER BY Receipt.date DESC;").unwrap();
        store_query.query_map([], |row|
            Ok(ReceiptRow{
                id: row.get(0)?,
                store: row.get(1)?,
                date: row.get(2)?,
                store_name: row.get(3)?,
            })
        ).unwrap().filter_map(|row| row.ok()).collect()
    };

    let model = App {
        conn,
        stores,
        receipts,
    };

    let app = RelmApp::new(model);
    app.run();
}
