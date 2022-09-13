use crate::analysis::{ColumnType, RowData};
use crate::AppendAll;
use relm4::factory::{
    DynamicIndex, FactoryComponent, FactoryComponentSender, FactoryVecDeque, FactoryVecDequeGuard,
};
use relm4::gtk::glib::GString;
use relm4::gtk::{self, prelude::*};
use relm4::{ComponentParts, ComponentSender, SimpleComponent};
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
    fn new(name: String, ty: ColumnType) -> Self {
        Row {
            name,
            ty,
            duplicate: false,
            up: true,
            down: true,
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
    type Init = (String, ColumnType);
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
            gtk::Entry::builder().text(&self.name).build() {
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
        (name, ty): Self::Init,
        _index: &DynamicIndex,
        _sender: FactoryComponentSender<Self>,
    ) -> Self {
        Row::new(name, ty)
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
    NotEnoughRows,
    NotFilled,
    Duplicates,
    Valid,
}

pub(crate) struct Type {
    ty: FactoryVecDeque<Row>,
    is_filled: bool,
    /// This field may only contain a useful value if [`Type::is_filled`] is true
    has_duplicates: bool,
    required_rows: usize,
}

impl Type {
    pub(crate) fn get_row_data(&self) -> RowData {
        RowData(
            self.ty
                .iter()
                .map(|row| (row.name.trim().to_string(), row.ty))
                .collect(),
        )
    }
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

#[relm4::component(pub(crate))]
impl SimpleComponent for Type {
    type Input = TypeMsg;
    type Output = ValidityMsg;
    type Init = usize;
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
        init: Self::Init,
        root: &Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let row_box = gtk::Box::default();

        let ty = FactoryVecDeque::new(row_box.clone(), &sender.input);

        let model = Type {
            ty,
            is_filled: false,
            has_duplicates: false,
            required_rows: init,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        let mut types = self.ty.guard();
        let send = |val: Validity| {
            sender.output(ValidityMsg::ValidityChanged(val));
        };
        match message {
            TypeMsg::Add => {
                types.push_back((String::new(), ColumnType::String));
                types.restore_move_valid();
                if self.is_filled {
                    send(Validity::NotFilled);
                    self.is_filled = false;
                }
            }
            TypeMsg::AddAbove(idx) => {
                let idx = idx.current_index();
                types.insert(idx, (String::new(), ColumnType::String));
                types.restore_move_valid();
                if self.is_filled {
                    send(Validity::NotFilled);
                    self.is_filled = false;
                }
            }
            TypeMsg::Delete(idx) => {
                let idx = idx.current_index();
                types.remove(idx);
                types.restore_move_valid();
                if types.len() < self.required_rows {
                    send(Validity::NotEnoughRows);
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
                                send(Validity::Duplicates);
                                self.has_duplicates = true;
                            } else {
                                send(Validity::Valid);
                                self.has_duplicates = false;
                            }
                            self.is_filled = true;
                        }
                        (true, false) => {
                            panic!("deleting a row should not be able to make another row empty.");
                        }
                        (true, true) => {
                            if self.has_duplicates && !has_duplicates {
                                send(Validity::Valid);
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
                    if self.has_duplicates {
                        types.check_duplicates();
                    }
                }
            }
            TypeMsg::MoveDown(idx) => {
                let idx = idx.current_index();
                let new_idx = idx + 1;
                if new_idx < types.len() {
                    types.move_to(idx, new_idx);
                    types.restore_move_valid();
                    if self.has_duplicates {
                        types.check_duplicates();
                    }
                }
            }
            TypeMsg::Replicate(row_data) => {
                types.clear();
                for (name, ty) in row_data.0 {
                    types.push_back((name, ty));
                }
                types.restore_move_valid();

                self.is_filled = types.is_filled();
                self.has_duplicates = types.check_duplicates();
                if types.len() < self.required_rows {
                    send(Validity::NotEnoughRows);
                } else if !self.is_filled {
                    send(Validity::NotFilled);
                } else if self.has_duplicates {
                    send(Validity::Duplicates);
                } else {
                    send(Validity::Valid);
                }
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
                            send(Validity::NotFilled);
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
                                send(Validity::Duplicates);
                                self.has_duplicates = true;
                            } else {
                                // valid
                                send(Validity::Valid);
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
                                    send(Validity::Duplicates);
                                    self.has_duplicates = true;
                                }
                                (true, false) => {
                                    send(Validity::Valid);
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
