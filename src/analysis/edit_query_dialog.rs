use crate::analysis::type_def::TypeDef;
use crate::analysis::{AnalysisModel, ColumnType, Query};
use crate::AnalysisMsg;
use gtk::glib::GString;
use gtk::prelude::*;
use relm4::{send, ComponentUpdate, Model, Sender, WidgetPlus, Widgets};

#[tracker::track]
struct Ui {
    name_valid: bool,
    #[tracker::no_eq]
    ok_button_name: String,
    #[tracker::no_eq]
    init_query: Vec<(String, ColumnType)>,
    #[tracker::no_eq]
    name: String,
    #[tracker::no_eq]
    sql: String,
}

pub(crate) struct QueryDialogModel {
    hidden: bool,
    id: usize,
    names: Vec<String>,
    ui: Ui,
}

pub(crate) enum QueryDialogMsg {
    Open {
        query: Query,
        id: usize,
        names: Vec<String>,
        ok_button_name: String,
    },
    Accept(Query, String),
    Cancel,
    NameChanged(GString),
}

impl Model for QueryDialogModel {
    type Msg = QueryDialogMsg;
    type Widgets = QueryDialogWidgets;
    type Components = ();
}

impl ComponentUpdate<AnalysisModel> for QueryDialogModel {
    fn init_model(_parent_model: &AnalysisModel) -> Self {
        QueryDialogModel {
            hidden: true,
            id: 0,
            names: Vec::new(),
            ui: Ui {
                name_valid: false,
                ok_button_name: String::new(),
                init_query: Vec::new(),
                name: "".to_string(),
                sql: "".to_string(),
                tracker: 0,
            },
        }
    }

    fn update(
        &mut self,
        msg: QueryDialogMsg,
        _components: &(),
        _sender: Sender<QueryDialogMsg>,
        parent_sender: Sender<AnalysisMsg>,
    ) {
        self.ui.reset();
        match msg {
            QueryDialogMsg::Open {
                query,
                names,
                id,
                ok_button_name,
            } => {
                let current_name = &names[id];

                self.hidden = false;
                self.id = id;
                self.ui.set_init_query(query.table_header);
                self.ui.set_name_valid(!current_name.is_empty());
                self.ui.set_ok_button_name(ok_button_name);
                self.ui.set_name(current_name.clone());
                self.ui.set_sql(query.sql);
                self.names = names;
            }
            QueryDialogMsg::Accept(query, name) => {
                self.hidden = true;
                send!(
                    parent_sender,
                    AnalysisMsg::EditQueryResult(query, name, self.id)
                );
            }
            QueryDialogMsg::Cancel => {
                self.hidden = true;
            }
            QueryDialogMsg::NameChanged(name) => {
                let name = name.trim();
                self.ui.set_name_valid(
                    !name.is_empty()
                        && !self
                            .names
                            .iter()
                            .enumerate()
                            .filter(|(i, _)| *i != self.id)
                            .any(|(_, n)| n == name),
                );
            }
        }
    }
}

trait SetContent {
    fn append_content(&self, widget: &impl IsA<gtk::Widget>);
}

impl SetContent for gtk::Dialog {
    fn append_content(&self, widget: &impl IsA<gtk::Widget>) {
        self.content_area().append(widget);
    }
}

#[relm4_macros::widget(pub(crate))]
impl Widgets<QueryDialogModel, AnalysisModel> for QueryDialogWidgets {
    view! {
        dialog = gtk::Dialog {
            set_modal: true,
            set_default_width: 1300,
            set_visible: watch!(!model.hidden),

            append_content: name_entry = &gtk::Entry {
                set_hexpand: false,
                set_halign: gtk::Align::Center,
                set_text: track!(model.ui.changed(Ui::name()), model.ui.name.as_str()),

                connect_changed(sender) => move |name| {
                    send!(sender, QueryDialogMsg::NameChanged(name.text()));
                },
            },
            append_content: sql_entry = &gtk::Entry {
                set_hexpand: true,
                set_halign: gtk::Align::Fill,
                set_text: track!(model.ui.changed(Ui::sql()), model.ui.sql.as_str()),
            },
            append_content: headers = &TypeDef {
                set_hexpand: true,
                set_vexpand: true,
                set_halign: gtk::Align::Center,
                set_valign: gtk::Align::Center,
                set_orientation: gtk::Orientation::Horizontal,
                replicate: track!(model.ui.changed(Ui::init_query()), model.ui.init_query.as_slice()),
            },

            connect_response(sender, sql_entry, name_entry, headers) => move |_, resp| {
                let response = if resp == gtk::ResponseType::Accept {
                    let table_header = headers.row_data();
                    let name = name_entry.text();
                    let name = name.trim();
                    if !table_header.is_empty() && table_header.iter().all(|row| !row.0.is_empty()) && !name.is_empty(){
                        QueryDialogMsg::Accept(
                            Query {
                                sql: sql_entry.text().trim().to_string(),
                                table_header,
                            },
                            name.to_string()
                        )
                    } else {
                        QueryDialogMsg::Cancel
                    }
                } else {
                    QueryDialogMsg::Cancel
                };
                send!(sender, response);
            }
        }
    }

    fn post_init() {
        let content: gtk::Box = dialog.content_area();
        content.set_hexpand(true);
        content.set_vexpand(true);
        content.set_halign(gtk::Align::Fill);
        content.set_valign(gtk::Align::Center);
        content.set_orientation(gtk::Orientation::Vertical);
        content.set_margin_all(5);
        content.set_spacing(5);
        let add_button: gtk::Button = dialog
            .add_button("add", gtk::ResponseType::Accept)
            .downcast::<gtk::Button>()
            .unwrap();
        dialog.add_button("cancel", gtk::ResponseType::Cancel);
    }

    additional_fields! {
        add_button: gtk::Button,
    }

    fn post_view() {
        let model: &QueryDialogModel = model;

        if model.ui.changed(Ui::name_valid()) {
            add_button.set_sensitive(model.ui.name_valid);
        }
        if model.ui.changed(Ui::ok_button_name()) {
            add_button.set_label(model.ui.ok_button_name.as_str());
        }
    }

    fn post_connect_parent(&mut self, parent_widgets: &AnalysisWidgets) {
        self.dialog
            .set_transient_for(parent_widgets.main_window.as_ref());
    }
}
