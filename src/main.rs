use crate::unit::Unit;
use gtk::glib::{DateTime, GString, Sender};
use gtk::prelude::*;
use gtk::Align;
use relm4::{send, AppUpdate, Model, RelmApp, WidgetPlus, Widgets};
use relm4_macros::view;
use rusqlite::{params, Connection};

mod combobox;
mod unit;

use combobox::AppendAll;

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
}

struct App {
    conn: Connection,
    ui: Ui,
}

enum Msg {
    SelectUnit(Unit),
    AddStore(Store),
    AddReceipt(Receipt),
    AddItem(Item),
}

impl App {
    fn load_stores(&mut self) {
        let mut store_query = self
            .conn
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
            .or_else(|| new_stores.is_empty().then(|| new_stores.len() - 1))
            .map(|idx| idx as u32);
        self.ui.set_stores((new_stores, row_to_select));
    }

    fn load_receipts(&mut self) {
        let mut store_query = self.conn.prepare("SELECT Receipt.id, Receipt.date, Store.name FROM Receipt INNER JOIN Store ON Receipt.store = Store.id ORDER BY Receipt.id ASC;").unwrap();
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
            .or_else(|| new_receipts.is_empty().then(|| new_receipts.len() - 1))
            .map(|idx| idx as u32);
        self.ui.set_receipts((new_receipts, row_to_select));
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
            Msg::AddStore(store) => {
                let insert_query = self.conn.execute(
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
            Msg::AddReceipt(receipts) => {
                let store = &self.ui.stores.0[receipts.store_idx as usize];
                let insert_query = self.conn.execute(
                    "INSERT INTO Receipt (store, date) VALUES (?1, ?2);",
                    params![store.id, receipts.date.format("%F").unwrap().as_str()],
                );
                if let Err(err) = insert_query {
                    eprintln!("[add receipt]{err:#?}");
                } else {
                    self.load_receipts();
                }
            }
            Msg::AddItem(item) => {
                let receipt = &self.ui.receipts.0[item.receipt_idx as usize];
                let insert_query = self.conn.execute(
                    "INSERT INTO Item (name, quantity, price, unit, receipt) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![item.name.as_str(), item.quantity, item.price, item.unit.as_str(), receipt.id],
                );
                if let Err(err) = insert_query {
                    eprintln!("[add item]{err:#?}");
                } else {
                    self.ui.reset_item_fields = true;
                }
            }
            Msg::SelectUnit(unit) => self.ui.set_selected_unit(unit),
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
            set_default_width: 1300,
            set_title: Some("SQLBon"),
            set_child = Some(&gtk::Box) {
                set_orientation: gtk::Orientation::Vertical,
                append = &gtk::HeaderBar { },

                append: notebook = &gtk::Notebook {
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
                        },
                    },
                },
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
    let conn =
        Connection::open("/home/janek/Downloads/sqlite-tools-linux-x86-3360000/expenses-test")
            .unwrap();

    let mut model = App {
        conn,
        ui: Ui {
            selected_unit: Unit::NOK,
            stores: (Vec::new(), None),
            receipts: (Vec::new(), None),
            reset_item_fields: false,
            reset_store_fields: false,
            tracker: 0,
        },
    };
    model.load_stores();
    model.load_receipts();

    let app = RelmApp::new(model);
    app.run();
}
