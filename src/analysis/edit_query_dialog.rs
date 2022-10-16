use crate::analysis::type_component::{TypeMsg, Validity};
use crate::analysis::{type_component, Query, RowData};
use crate::dialog_ext::AppendDialog;
use crate::AnalysisMsg;
use relm4::gtk::glib::GString;
use relm4::gtk::prelude::*;
use relm4::{
    gtk, Component, ComponentController, ComponentParts, ComponentSender, Controller,
    RelmWidgetExt, SimpleComponent,
};

#[tracker::track]
struct Ui {
    name_valid: bool,
    output_valid: bool,
    input_valid: bool,
    #[tracker::no_eq]
    ok_button_name: String,
    #[tracker::no_eq]
    init_query: RowData,
    #[tracker::no_eq]
    name: String,
    #[tracker::no_eq]
    sql: String,
    #[tracker::no_eq]
    input_status: String,
    #[tracker::no_eq]
    output_status: String,
    #[tracker::no_eq]
    name_status: String,
}

pub(crate) struct QueryDialog {
    hidden: bool,
    id: usize,
    names: Vec<String>,
    ui: Ui,
    output_types: Controller<type_component::Type>,
    input_types: Controller<type_component::Type>,
}

#[derive(Debug)]
pub(crate) enum QueryDialogMsg {
    Open {
        query: Query,
        id: usize,
        names: Vec<String>,
        ok_button_name: String,
    },
    Accept {
        name: String,
        sql: String,
    },
    Cancel,
    NameChanged(GString),
    OutputValidityChanged(Validity),
    InputValidityChanged(Validity),
}

#[relm4::component(pub(crate))]
impl SimpleComponent for QueryDialog {
    type Input = QueryDialogMsg;
    type Output = AnalysisMsg;
    type Init = gtk::Window;
    type Widgets = QueryDialogWidgets;

    view! {
        #[root]
        #[name(dialog)]
        gtk::Dialog {
            set_transient_for: Some(&parent_window),
            set_modal: true,
            set_default_width: 1300,
            #[watch]
            set_visible: !model.hidden,
            append = &gtk::Box {
                set_hexpand: true,
                set_vexpand: true,
                set_halign: gtk::Align::Fill,
                set_valign: gtk::Align::Center,
                set_orientation: gtk::Orientation::Vertical,
                set_margin_all: 5,
                set_spacing: 5,

                gtk::Grid {
                    set_row_spacing: 5,
                    set_column_spacing: 7,
                    attach[0, 0, 1, 1] = &gtk::Label {
                        set_text: "Name:",
                        set_halign: gtk::Align::End,
                    },
                    attach[1, 0, 1, 1]: name_entry = &gtk::Entry {
                        set_hexpand: false,
                        set_halign: gtk::Align::Center,
                        #[track(model.ui.changed(Ui::name()))]
                        set_text: model.ui.name.as_str(),

                        connect_changed[sender] => move |name| {
                            sender.input(QueryDialogMsg::NameChanged(name.text()));
                        },
                    },
                    attach[0, 1, 1, 1] = &gtk::Label {
                        set_text: "SQL:",
                        set_halign: gtk::Align::End,
                    },
                    attach[1, 1, 1, 1]: sql_entry = &gtk::Entry {
                        set_hexpand: true,
                        set_halign: gtk::Align::Fill,
                        #[track(model.ui.changed(Ui::sql()))]
                        set_text: model.ui.sql.as_str(),
                    },
                    attach[1, 2, 1, 1] = &gtk::Label {
                        #[track(model.ui.changed(Ui::name_status()))]
                        set_text: model.ui.name_status.as_str(),
                        set_halign: gtk::Align::Center,
                    },
                    attach[0, 3, 2, 1] = &gtk::Separator {},
                    attach[0, 4, 1, 1] = &gtk::Label {
                        set_text: "Header Definition:",
                        set_halign: gtk::Align::End,
                    },
                    attach[1, 4, 1, 1]: model.output_types.widget(),
                    attach[1, 5, 1, 1] = &gtk::Label {
                        #[track(model.ui.changed(Ui::output_status()))]
                        set_text: model.ui.output_status.as_str(),
                        set_halign: gtk::Align::Center,
                    },
                    attach[0, 6, 2, 1] = &gtk::Separator {},
                    attach[0, 7, 1, 1] = &gtk::Label {
                        set_text: "Input Definition:",
                        set_halign: gtk::Align::End,
                    },
                    attach[1, 7, 1, 1]: model.input_types.widget(),
                    attach[1, 8, 1, 1] = &gtk::Label {
                        #[track(model.ui.changed(Ui::input_status()))]
                        set_text: model.ui.input_status.as_str(),
                        set_halign: gtk::Align::Center,
                    },
                },
            },
            connect_response[sender, sql_entry, name_entry] => move |_, resp| {
                let response = if resp == gtk::ResponseType::Accept {
                    let name = name_entry.text().trim().to_string();
                    let sql = sql_entry.text().trim().to_string();
                    QueryDialogMsg::Accept{
                        sql,
                        name,
                    }
                } else {
                    QueryDialogMsg::Cancel
                };
                sender.input(response);
            }
        }
    }

    additional_fields! {
        add_button: gtk::Button,
    }

    fn post_view() {
        let model: &QueryDialog = model;
        let add_button: &gtk::Button = add_button;

        if model
            .ui
            .changed(Ui::name_valid() | Ui::output_valid() | Ui::input_valid())
        {
            add_button.set_sensitive(
                model.ui.name_valid && model.ui.output_valid && model.ui.input_valid,
            );
        }
        if model.ui.changed(Ui::ok_button_name()) {
            add_button.set_label(model.ui.ok_button_name.as_str());
        }
    }

    fn init(
        parent_window: Self::Init,
        root: &Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let input_types =
            type_component::Type::builder()
                .launch(0)
                .forward(sender.input_sender(), |val_msg| match val_msg {
                    type_component::ValidityMsg::ValidityChanged(val) => {
                        QueryDialogMsg::InputValidityChanged(val)
                    }
                });
        let output_types =
            type_component::Type::builder()
                .launch(1)
                .forward(sender.input_sender(), |val_msg| match val_msg {
                    type_component::ValidityMsg::ValidityChanged(val) => {
                        QueryDialogMsg::OutputValidityChanged(val)
                    }
                });

        let model = QueryDialog {
            hidden: true,
            id: 0,
            names: Vec::new(),
            ui: Ui {
                name_valid: false,
                output_valid: false,
                input_valid: false,
                ok_button_name: String::new(),
                init_query: RowData::new(),
                name: String::new(),
                sql: String::new(),
                input_status: String::new(),
                output_status: String::new(),
                name_status: String::new(),
                tracker: 0,
            },
            output_types,
            input_types,
        };

        // this is a place-holder to generate the widgets struct. It is replaced shortly after.
        let add_button = gtk::Button::new();

        let mut widgets = view_output!();
        widgets.add_button = widgets
            .dialog
            .add_button("add", gtk::ResponseType::Accept)
            .downcast::<gtk::Button>()
            .unwrap();
        widgets
            .dialog
            .add_button("cancel", gtk::ResponseType::Cancel);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: QueryDialogMsg, sender: ComponentSender<Self>) {
        self.ui.reset();
        match message {
            QueryDialogMsg::Open {
                query,
                names,
                id,
                ok_button_name,
            } => {
                let current_name = &names[id];

                self.hidden = false;
                self.id = id;
                self.ui.set_init_query(query.table_header.clone());
                self.ui.set_name_valid(!current_name.is_empty());
                self.ui.set_ok_button_name(ok_button_name);
                self.ui.set_name(current_name.clone());
                self.ui.set_sql(query.sql);
                self.output_types
                    .emit(TypeMsg::Replicate(query.table_header));
                self.input_types.emit(TypeMsg::Replicate(query.query_input));
                self.names = names;
            }
            QueryDialogMsg::Accept { name, sql } => {
                if self.ui.input_valid && self.ui.output_valid {
                    let table_header = self.output_types.state().get().model.get_row_data();
                    let query_input = self.input_types.state().get().model.get_row_data();
                    let query = Query {
                        sql,
                        table_header,
                        query_input,
                    };
                    sender.output(AnalysisMsg::EditQueryResult(query, name, self.id));
                    self.hidden = true;
                }
            }
            QueryDialogMsg::Cancel => {
                self.hidden = true;
            }
            QueryDialogMsg::NameChanged(name) => {
                let name = name.trim();
                let is_filled = !name.is_empty();
                if is_filled {
                    let is_name_unique = self
                        .names
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| *i != self.id)
                        .all(|(_, n)| n != name);
                    if is_name_unique {
                        self.ui.set_name_status(String::new());
                        self.ui.set_name_valid(true);
                    } else {
                        self.ui
                            .set_name_status("This name is not unique.".to_string());
                        self.ui.set_name_valid(false);
                    }
                } else {
                    self.ui
                        .set_name_status("Each query needs a name.".to_string());
                    self.ui.set_name_valid(false);
                }
            }
            QueryDialogMsg::InputValidityChanged(val) => {
                self.ui.set_input_valid(val == Validity::Valid);
                match val {
                    Validity::NotEnoughRows => {}
                    Validity::NotFilled => self
                        .ui
                        .set_input_status("All query input entries need a name.".to_string()),
                    Validity::Duplicates => self
                        .ui
                        .set_input_status("All query input entries need to be unique.".to_string()),
                    Validity::Valid => self.ui.set_input_status(String::new()),
                }
            }
            QueryDialogMsg::OutputValidityChanged(val) => {
                self.ui.set_output_valid(val == Validity::Valid);
                match val {
                    Validity::NotEnoughRows => self.ui.set_output_status(
                        "At least one table header entry is required.".to_string(),
                    ),
                    Validity::NotFilled => self
                        .ui
                        .set_output_status("All table header entries need a name.".to_string()),
                    Validity::Duplicates => self.ui.set_output_status(
                        "All table header entries need to be unique.".to_string(),
                    ),
                    Validity::Valid => self.ui.set_output_status(String::new()),
                }
            }
        }
    }
}
