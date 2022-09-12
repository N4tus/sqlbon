use crate::analysis::{ColumnType, RowData};
use crate::AppendAll;
use relm4::factory::{
    DynamicIndex, FactoryComponent, FactoryComponentSender, FactoryVecDeque, FactoryVecDequeGuard,
};
use relm4::gtk::glib::GString;
use relm4::gtk::{self, prelude::*};
use relm4::{
    ComponentController, ComponentParts, ComponentSender, Controller, RelmIterChildrenExt,
    SimpleComponent,
};
use std::collections::HashSet;
use std::fmt::Debug;

#[derive(Debug)]
struct Row {
    name: String,
    ty: ColumnType,
    duplicate: bool,
    up: bool,
    down: bool,
}

#[derive(Debug)]
enum RowMsg {
    NameChanged(DynamicIndex, bool),
    AddAbove(DynamicIndex),
    Delete(DynamicIndex),
    MoveUp(DynamicIndex),
    MoveDown(DynamicIndex),
}

impl Row {
    fn new() -> Self {
        Row {
            name: String::new(),
            ty: ColumnType::String,
            duplicate: false,
            up: false,
            down: false,
        }
    }
}

#[derive(Debug)]
enum RowValid {
    NameChanged(DynamicIndex, GString),
    TypeChanged(ColumnType),
}

#[relm4::factory]
impl FactoryComponent for Row {
    type CommandOutput = ();
    type Init = ();
    type Input = RowValid;
    type Output = RowMsg;
    type ParentInput = TypeMsg;
    type ParentWidget = gtk::Box;
    type Widgets = RowWidgets;

    view! {
        #[root]
        gtk::Box{
            set_orientation: gtk::Orientation::Horizontal,
            #[name(name_entry)]
            gtk::Entry {
                connect_changed[sender, index] => move |name_entry| {
                    sender.input(RowValid::NameChanged(index.clone(), name_entry.text()));
                },
            },
            gtk::ComboBoxText {
                set_size_request: (100, -1),
                append_all_and_select: (
                    [
                        ColumnType::String.to_string(),
                        ColumnType::Number.to_string(),
                        ColumnType::Date.to_string(),
                    ],
                    Some(0),
                ),
                connect_changed[sender] => move |type_box| {
                    sender.input(RowValid::TypeChanged(type_box.active().unwrap().try_into().unwrap()));
                },
            },
            gtk::Button {
                set_label: "new",
                connect_clicked[sender, index] => move |_| {
                    sender.output(RowMsg::AddAbove(index.clone()));
                },
            },
            gtk::Button {
                set_label: "delete",
                connect_clicked[sender, index] => move |_| {
                    sender.output(RowMsg::Delete(index.clone()));
                },
            },
            gtk::Button {
                set_label: "up",
                #[watch]
                set_sensitive: self.up,
                connect_clicked[sender, index] => move |_| {
                    sender.output(RowMsg::MoveUp(index.clone()));
                },
            },
            gtk::Button {
                set_label: "down",
                #[watch]
                set_sensitive: self.down,
                connect_clicked[sender, index] => move |_| {
                    sender.output(RowMsg::MoveDown(index.clone()));
                },
            },
        }
    }

    fn output_to_parent_input(output: RowMsg) -> Option<TypeMsg> {
        Some(match output {
            RowMsg::NameChanged(index, prev_not_empty) => {
                TypeMsg::NameChanged(index, prev_not_empty)
            }
            RowMsg::AddAbove(index) => TypeMsg::AddAbove(index),
            RowMsg::Delete(index) => TypeMsg::Delete(index),
            RowMsg::MoveUp(index) => TypeMsg::MoveUp(index),
            RowMsg::MoveDown(index) => TypeMsg::MoveDown(index),
        })
    }

    fn init_model(
        _value: Self::Init,
        _index: &DynamicIndex,
        _sender: FactoryComponentSender<Self>,
    ) -> Self {
        Row::new()
    }

    fn update(&mut self, message: Self::Input, sender: FactoryComponentSender<Self>) {
        match message {
            RowValid::NameChanged(index, s) => {
                let prev_not_empty = !self.name.trim().is_empty();
                self.name = s.to_string();
                sender.output(RowMsg::NameChanged(index, prev_not_empty));
            }
            RowValid::TypeChanged(ty) => {
                self.ty = ty;
            }
        }
    }

    fn pre_view() {
        let name_entry: &gtk::Entry = &widgets.name_entry;
        if self.duplicate {
            name_entry.add_css_class("duplicate-name");
        } else {
            name_entry.remove_css_class("duplicate-name");
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub(crate) enum Validity {
    NoRows,
    NotFilled,
    Duplicates,
    Valid,
}

pub(crate) struct Type {
    ty: FactoryVecDeque<Row>,
    set_row_data: Option<RowData>,
    is_filled: bool,
    /// This field may only contain a useful value if [`Type::is_filled`] is true
    has_duplicates: bool,
}

#[derive(Debug)]
pub(crate) enum TypeMsg {
    Add,
    NameChanged(DynamicIndex, bool),
    AddAbove(DynamicIndex),
    Delete(DynamicIndex),
    MoveUp(DynamicIndex),
    MoveDown(DynamicIndex),
    Replicate(RowData),
}

trait RestoreMoveValid {
    fn restore_move_valid(&mut self);
    fn check_duplicates(&mut self) -> bool;
    fn is_filled(&self) -> bool;
}

impl RestoreMoveValid for FactoryVecDequeGuard<'_, Row> {
    fn restore_move_valid(&mut self) {
        for &(idx, up, down) in match self.len() {
            0 => return,
            1 => &[(0, false, false)] as &'static [(i32, bool, bool)],
            2 => &[(0, false, true), (1, true, false)],
            3 => &[(0, false, true), (1, true, true), (2, true, false)],
            _ => &[
                (0, false, true),
                (1, true, true),
                (-2, true, true),
                (-1, true, false),
            ],
        } {
            let idx = if idx < 0 {
                self.len() as i32 + idx
            } else {
                idx
            } as usize;
            let row = self.get_mut(idx).unwrap();
            row.up = up;
            row.down = down;
        }
    }

    fn check_duplicates(&mut self) -> bool {
        let mut has_duplicates = false;
        let mut dup_map = HashSet::new();
        let mut dup_vec = Vec::new();
        for (i, row) in self.iter().enumerate() {
            let name = row.name.trim();
            let is_duplicate = !name.is_empty() && !dup_map.insert(name);
            if row.duplicate != is_duplicate {
                dup_vec.push((i, is_duplicate));
            }
            has_duplicates |= is_duplicate;
        }
        for (dup_idx, is_duplicate) in dup_vec {
            self.get_mut(dup_idx).unwrap().duplicate = is_duplicate;
        }
        has_duplicates
    }

    fn is_filled(&self) -> bool {
        self.iter().all(|row| !row.name.trim().is_empty())
    }
}

#[derive(Debug)]
pub(crate) enum ValidityMsg {
    ValidityChanged(Validity),
}

pub(crate) trait GetRowData {
    fn get_row_data(&self) -> RowData;
}

impl GetRowData for Controller<Type> {
    fn get_row_data(&self) -> RowData {
        let rows = self
            .widget()
            .first_child()
            .unwrap()
            .downcast::<gtk::Box>()
            .unwrap()
            .iter_children()
            .map(|ch| {
                let row = ch.downcast::<gtk::Box>().unwrap();
                let name = row.first_child().unwrap();
                let ty = name.next_sibling().unwrap();
                let name = name.downcast::<gtk::Entry>().unwrap();
                let ty = ty.downcast::<gtk::ComboBoxText>().unwrap();
                let name = name.text().trim().to_string();
                let ty = ty.active().unwrap().try_into().unwrap();
                (name, ty)
            })
            .collect();
        RowData(rows)
    }
}

trait Replicate {
    fn replicate(&self, row_data: &RowData);
}

impl Replicate for gtk::Box {
    fn replicate(&self, row_data: &RowData) {
        for (row_box, row) in self.iter_children().zip(&row_data.0) {
            let row_box = row_box.downcast::<gtk::Box>().unwrap();
            let name = row_box.first_child().unwrap();
            let ty = name.next_sibling().unwrap();
            let name = name.downcast::<gtk::Entry>().unwrap();
            let ty = ty.downcast::<gtk::ComboBoxText>().unwrap();

            name.set_text(row.0.as_str());
            ty.set_active(Some(row.1.into()));
        }
    }
}

#[relm4::component(pub(crate))]
impl SimpleComponent for Type {
    type Input = TypeMsg;
    type Output = ValidityMsg;
    type Init = ();
    type Widgets = TypeWidgets;

    view! {
        #[root]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_hexpand: true,
            set_vexpand: true,
            set_halign: gtk::Align::Center,
            set_valign: gtk::Align::Center,
            #[local]
            row_box -> gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                #[watch]
                replicate?: &model.set_row_data,
            },
            gtk::Button {
                set_label: "new",
                connect_clicked[sender] => move |_| {
                    sender.input(TypeMsg::Add);
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: &Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let row_box = gtk::Box::default();

        let ty = FactoryVecDeque::new(row_box.clone(), &sender.input);

        let model = Type {
            ty,
            set_row_data: None,
            is_filled: false,
            has_duplicates: false,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        self.set_row_data = None;
        let mut types = self.ty.guard();
        match message {
            TypeMsg::Add => {
                types.push_back(());
                types.restore_move_valid();
                if self.is_filled {
                    sender.output(ValidityMsg::ValidityChanged(Validity::NotFilled));
                    self.is_filled = false;
                }
            }
            TypeMsg::AddAbove(idx) => {
                let idx = idx.current_index();
                types.insert(idx, ());
                types.restore_move_valid();
                if self.is_filled {
                    sender.output(ValidityMsg::ValidityChanged(Validity::NotFilled));
                    self.is_filled = false;
                }
            }
            TypeMsg::Delete(idx) => {
                let idx = idx.current_index();
                types.remove(idx);
                types.restore_move_valid();
                if types.is_empty() {
                    sender.output(ValidityMsg::ValidityChanged(Validity::NoRows));
                    self.is_filled = false;
                } else {
                    // if filled, deleting wont empty a row

                    let has_duplicates = types.check_duplicates();
                    let is_filled = types.is_filled();

                    //  n n => do nothing
                    //  n f => check dup[emit dup/emit valid]
                    //  f n => impossible
                    //  f f => if dup: check dup[emit valid if not]
                    match (self.is_filled, is_filled) {
                        (false, false) => {}
                        (false, true) => {
                            if has_duplicates {
                                sender.output(ValidityMsg::ValidityChanged(Validity::Duplicates));
                                self.has_duplicates = true;
                            } else {
                                sender.output(ValidityMsg::ValidityChanged(Validity::Valid));
                                self.has_duplicates = false;
                            }
                            self.is_filled = true;
                        }
                        (true, false) => {
                            panic!("deleting a row should not be able to make another row empty.");
                        }
                        (true, true) => {
                            if self.has_duplicates && !has_duplicates {
                                sender.output(ValidityMsg::ValidityChanged(Validity::Valid));
                                self.has_duplicates = false;
                            }
                        }
                    }
                }
            }
            TypeMsg::MoveUp(idx) => {
                let idx = idx.current_index();
                if let Some(new_idx) = idx.checked_sub(1) {
                    types.move_to(idx, new_idx);
                    types.restore_move_valid();
                }
            }
            TypeMsg::MoveDown(idx) => {
                let idx = idx.current_index();
                let new_idx = idx + 1;
                if new_idx < types.len() {
                    types.move_to(idx, new_idx);
                    types.restore_move_valid();
                }
            }
            TypeMsg::Replicate(row_data) => {
                types.clear();
                for _ in &row_data.0 {
                    types.push_back(());
                }
                self.set_row_data = Some(row_data)
            }
            TypeMsg::NameChanged(idx, prev_not_empty) => {
                let idx = idx.current_index();
                let name = &types.get(idx).unwrap().name;
                let current_not_empty = !name.trim().is_empty();
                let has_duplicates = types.check_duplicates();

                match (prev_not_empty, current_not_empty) {
                    (false, false) => {
                        // still empty, do nothing
                    }
                    (true, false) => {
                        // now empty
                        if self.is_filled {
                            sender.output(ValidityMsg::ValidityChanged(Validity::NotFilled));
                            self.is_filled = false;
                        }
                    }
                    (false, true) => {
                        // now filled, but maybe duplicate
                        if types.is_filled() {
                            // no other fields empty
                            self.is_filled = true;
                            if has_duplicates {
                                // duplicates
                                sender.output(ValidityMsg::ValidityChanged(Validity::Duplicates));
                                self.has_duplicates = true;
                            } else {
                                // valid
                                sender.output(ValidityMsg::ValidityChanged(Validity::Valid));
                                self.has_duplicates = false;
                            }
                        }
                    }
                    (true, true) => {
                        // still filled, but maybe duplicate
                        // it is either, valid or not filled
                        if self.is_filled {
                            match (self.has_duplicates, has_duplicates) {
                                (false, true) => {
                                    sender
                                        .output(ValidityMsg::ValidityChanged(Validity::Duplicates));
                                    self.has_duplicates = true;
                                }
                                (true, false) => {
                                    sender.output(ValidityMsg::ValidityChanged(Validity::Valid));
                                    self.has_duplicates = false;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }
}
