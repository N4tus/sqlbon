use crate::{App, DateTime, Msg, Store, StoreRow};
use gtk::prelude::*;
use relm4::{send, ComponentUpdate, Model, Sender, Widgets};

pub(crate) enum WarningOrigin {
    Receipt { store: StoreRow, date: DateTime },
    Store { name: String, location: String },
}

pub(crate) struct Dialog {
    hidden: bool,
    origin: WarningOrigin,
}

pub(crate) enum DialogMsg {
    Show(WarningOrigin),
    Accept,
    Cancel,
}

impl Model for Dialog {
    type Msg = DialogMsg;
    type Widgets = DialogWidgets;
    type Components = ();
}

impl ComponentUpdate<App> for Dialog {
    fn init_model(_parent_model: &App) -> Self {
        Dialog {
            hidden: true,
            origin: WarningOrigin::Receipt {
                store: StoreRow {
                    id: 0,
                    name: String::new(),
                    location: String::new(),
                },
                date: DateTime::now_utc().unwrap(),
            },
        }
    }

    fn update(
        &mut self,
        msg: DialogMsg,
        _components: &(),
        _sender: Sender<DialogMsg>,
        parent_sender: Sender<Msg>,
    ) {
        match msg {
            DialogMsg::Show(origin) => {
                self.hidden = false;
                self.origin = origin;
            }
            DialogMsg::Accept => {
                self.hidden = true;
                match &self.origin {
                    WarningOrigin::Receipt { store, date } => {
                        send!(
                            parent_sender,
                            Msg::ForceAddReceipt(store.id, date.format("%F").unwrap())
                        );
                    }
                    WarningOrigin::Store { name, location } => {
                        send!(
                            parent_sender,
                            Msg::ForceAddStore(Store {
                                name: name.as_str().into(),
                                location: location.as_str().into(),
                            })
                        )
                    }
                }
            }
            DialogMsg::Cancel => self.hidden = true,
        }
    }
}

#[relm4_macros::widget(pub(crate))]
impl Widgets<Dialog, App> for DialogWidgets {
    view! {
        dialog = gtk::MessageDialog {
            set_modal: true,
            set_visible: watch!(!model.hidden),
            set_text: track!(!model.hidden, Some(&match &model.origin {
                WarningOrigin::Receipt{ store, date } => {
                    format!("A receipt for {} ({}) on {} already exists.", store.name, store.location, date.format("%F").unwrap().as_str())
                }
                WarningOrigin::Store{name, location} => {
                    format!("A store {} at {} already exists.", name, location)
                }
            })),
            set_secondary_text: match &model.origin {
                WarningOrigin::Receipt{ .. } => {
                    Some("It is uncommon to have two receipts for the same store on the same day. Do you really want to add this receipt?")
                }
                WarningOrigin::Store{ .. } => {
                    Some("It is uncommon to have two stores with the same name at the same location. Do you really want to add this store?")
                }
            },
            add_button: args!("Add", gtk::ResponseType::Accept),
            add_button: args!("Cancel", gtk::ResponseType::Cancel),
            connect_response(sender) => move |_, resp| {
                send!(sender, if resp == gtk::ResponseType::Accept {
                    DialogMsg::Accept
                } else {
                    DialogMsg::Cancel
                });
            }
        }
    }
    fn post_connect_parent(&mut self, parent_widgets: &AppWidgets) {
        self.dialog
            .set_transient_for(Some(&parent_widgets.main_window));
    }
}
