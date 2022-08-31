use crate::{App, DateTime, Msg, StoreRow};
use gtk::prelude::*;
use relm4::{send, ComponentUpdate, Model, Sender, Widgets};

pub(crate) struct Dialog {
    hidden: bool,
    store: StoreRow,
    date: DateTime,
}

pub(crate) enum DialogMsg {
    Show(StoreRow, DateTime),
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
            store: StoreRow {
                id: 0,
                name: String::new(),
                location: String::new(),
            },
            date: DateTime::now_utc().unwrap(),
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
            DialogMsg::Show(store, date) => {
                *self = Dialog {
                    hidden: false,
                    store,
                    date,
                }
            }
            DialogMsg::Accept => {
                self.hidden = true;
                send!(
                    parent_sender,
                    Msg::ForceAddReceipt(self.store.id, self.date.format("%F").unwrap())
                );
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
            set_text: track!(!model.hidden, Some(&format!("A receipt for {} ({}) on {} already exists.", model.store.name, model.store.location, model.date.format("%F").unwrap().as_str()))),
            set_secondary_text: Some("It is uncommon to have to receipts for the same store on the same day. Do you really want to add this receipt?"),
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
    fn post_connect_parent(&mut self, parent_widgets: &relm4::traits::Widgets) {
        self.dialog
            .set_transient_for(Some(&parent_widgets.main_window));
    }
}
