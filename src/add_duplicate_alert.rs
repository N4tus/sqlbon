use crate::{DateTime, Msg, Store, StoreRow};
use gtk::prelude::*;
use relm4::gtk;
use relm4::{ComponentParts, ComponentSender, SimpleComponent};

#[derive(Debug)]
pub(crate) enum WarningOrigin {
    Receipt { store: StoreRow, date: DateTime },
    Store { name: String, location: String },
}

pub(crate) struct Dialog {
    hidden: bool,
    origin: WarningOrigin,
}

#[derive(Debug)]
pub(crate) enum DialogMsg {
    Show(WarningOrigin),
    Accept,
    Cancel,
}

#[relm4::component(pub(crate))]
impl SimpleComponent for Dialog {
    type Input = DialogMsg;
    type Output = Msg;
    type Init = gtk::Window;
    type Widgets = DialogWidgets;

    view! {
        #[root]
        #[name(dialog)]
        gtk::MessageDialog {
            set_modal: true,
            set_transient_for: Some(&parent_window),
            #[watch]
            set_visible: !model.hidden,
            #[track(!model.hidden)]
            set_text: Some(&match &model.origin {
                WarningOrigin::Receipt{ store, date } => {
                    format!("A receipt for {} ({}) on {} already exists.", store.name, store.location, date.format("%F").unwrap().as_str())
                }
                WarningOrigin::Store{name, location} => {
                    format!("A store {} at {} already exists.", name, location)
                }
            }),
            #[track(!model.hidden)]
            set_secondary_text: match &model.origin {
                WarningOrigin::Receipt{ .. } => {
                    Some("It is uncommon to have two receipts for the same store on the same day. Do you really want to add this receipt?")
                }
                WarningOrigin::Store{ .. } => {
                    Some("It is uncommon to have two stores with the same name at the same location. Do you really want to add this store?")
                }
            },
            add_button: ("Add", gtk::ResponseType::Accept),
            add_button: ("Cancel", gtk::ResponseType::Cancel),
            connect_response[sender] => move |_, resp| {
                sender.input(if resp == gtk::ResponseType::Accept {
                    DialogMsg::Accept
                } else {
                    DialogMsg::Cancel
                });
            }
        }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            DialogMsg::Show(origin) => {
                self.hidden = false;
                self.origin = origin;
            }
            DialogMsg::Accept => {
                self.hidden = true;
                match &self.origin {
                    WarningOrigin::Receipt { store, date } => {
                        sender.output(Msg::ForceAddReceipt(store.id, date.format("%F").unwrap()));
                    }
                    WarningOrigin::Store { name, location } => {
                        sender.output(Msg::ForceAddStore(Store {
                            name: name.as_str().into(),
                            location: location.as_str().into(),
                        }));
                    }
                }
            }
            DialogMsg::Cancel => self.hidden = true,
        }
    }

    fn init(
        parent_window: Self::Init,
        root: &Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Dialog {
            hidden: true,
            origin: WarningOrigin::Receipt {
                store: StoreRow {
                    id: 0,
                    name: String::new(),
                    location: String::new(),
                },
                date: DateTime::now_utc().unwrap(),
            },
        };

        let widgets = view_output!();
        ComponentParts { model, widgets }
    }
}
