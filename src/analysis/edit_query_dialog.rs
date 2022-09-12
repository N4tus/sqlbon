use crate::analysis::type_component::{TypeMsg, Validity};
use crate::analysis::type_def::TypeDef;
use crate::analysis::{type_component, Query, RowData};
use crate::AnalysisMsg;
use relm4::gtk::glib::GString;
use relm4::gtk::prelude::*;
use relm4::{
    gtk, Component, ComponentController, ComponentParts, ComponentSender, Controller,
    SimpleComponent, WidgetPlus,
};

#[tracker::track]
struct Ui {
    name_valid: bool,
    #[tracker::no_eq]
    ok_button_name: String,
    #[tracker::no_eq]
    init_query: RowData,
    #[tracker::no_eq]
    name: String,
    #[tracker::no_eq]
    sql: String,
    #[tracker::no_eq]
    status: String,
}

pub(crate) struct QueryDialog {
    hidden: bool,
    id: usize,
    names: Vec<String>,
    ui: Ui,
    type_component: Controller<type_component::Type>,
}

#[derive(Debug)]
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
    ValidityChanged(Validity),
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
            gtk::Box {
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
                    attach[0, 2, 2, 1] = &gtk::Separator {},
                    attach[0, 3, 1, 1] = &gtk::Label {
                        set_text: "Result Header Definition:",
                        set_halign: gtk::Align::End,
                    },
                    attach[1, 3, 1, 1]: headers = &TypeDef {
                        set_hexpand: true,
                        set_vexpand: true,
                        set_halign: gtk::Align::Center,
                        set_valign: gtk::Align::Center,
                        set_orientation: gtk::Orientation::Horizontal,
                        #[track(model.ui.changed(Ui::init_query()))]
                        replicate: &model.ui.init_query,
                    },
                    attach[0, 4, 2, 1] = &gtk::Separator {},
                    attach[0, 5, 1, 1] = &gtk::Label {
                        set_text: "Input Definition:",
                        set_halign: gtk::Align::End,
                    },
                    attach[1, 5, 1, 1]: model.type_component.widget(),
                    attach[0, 6, 2, 1] = &gtk::Separator {},
                },
                gtk::Label {
                    #[track(model.ui.changed(Ui::status()))]
                    set_text: model.ui.status.as_str(),
                    set_halign: gtk::Align::Center,
                }
            },
            connect_response[sender, sql_entry, name_entry, headers] => move |_, resp| {
                let response = if resp == gtk::ResponseType::Accept {
                    let table_header = headers.row_data();
                    let name = name_entry.text();
                    let name = name.trim();
                    QueryDialogMsg::Accept(
                        Query {
                            sql: sql_entry.text().trim().to_string(),
                            table_header,
                        },
                        name.to_string()
                    )
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

        if model.ui.changed(Ui::name_valid()) {
            add_button.set_sensitive(model.ui.name_valid);
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
        let type_component =
            type_component::Type::builder()
                .launch(())
                .forward(sender.input_sender(), |val_msg| match val_msg {
                    type_component::ValidityMsg::ValidityChanged(val) => {
                        QueryDialogMsg::ValidityChanged(val)
                    }
                });

        let model = QueryDialog {
            hidden: true,
            id: 0,
            names: Vec::new(),
            ui: Ui {
                name_valid: false,
                ok_button_name: String::new(),
                init_query: RowData::new(),
                name: String::new(),
                sql: String::new(),
                status: String::new(),
                tracker: 0,
            },
            type_component,
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
                self.type_component
                    .emit(TypeMsg::Replicate(query.table_header));
                self.names = names;
            }
            QueryDialogMsg::Accept(query, name) => {
                if name.is_empty() {
                    self.ui.set_status("Each query needs a name.".to_string());
                } else if self.names.contains(&name) {
                    self.ui.set_status("This name is not unique.".to_string());
                } else if !query.table_header.has_entries() {
                    self.ui
                        .set_status("At least one table header entry is required.".to_string());
                } else if !query.table_header.is_filled() {
                    self.ui
                        .set_status("All table header entries need a name.".to_string());
                } else if !query.table_header.all_names_unique() {
                    self.ui
                        .set_status("All table header entries need to be unique.".to_string());
                } else {
                    self.hidden = true;
                    sender.output(AnalysisMsg::EditQueryResult(query, name, self.id));
                }
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
            QueryDialogMsg::ValidityChanged(val) => println!("{val:#?}"),
        }
    }
}
