use crate::analysis::ColumnType;
use crate::AppendAll;
use relm4::factory::{
    DynamicIndex, FactoryComponent, FactoryComponentSender, FactoryVecDeque, FactoryVecDequeGuard,
};
use relm4::gtk::{self, prelude::*};
use relm4::{ComponentParts, ComponentSender, SimpleComponent};
use std::fmt::Debug;

#[derive(Debug)]
struct Row {
    valid: bool,
    up: bool,
    down: bool,
}

#[derive(Debug)]
enum RowMsg {
    AddAbove(DynamicIndex),
    Delete(DynamicIndex),
    MoveUp(DynamicIndex),
    MoveDown(DynamicIndex),
}

impl Row {
    fn new() -> Self {
        Row {
            valid: false,
            up: false,
            down: false,
        }
    }
}

#[relm4::factory]
impl FactoryComponent for Row {
    type CommandOutput = ();
    type Init = ();
    type Input = ();
    type Output = RowMsg;
    type ParentInput = TypeMsg;
    type ParentWidget = gtk::Box;
    type Widgets = RowWidgets;

    view! {
        #[root]
        gtk::Box{
            set_orientation: gtk::Orientation::Horizontal,
            gtk::Entry {
                connect_changed[sender] => move |name_entry| {
                    // sender.output(name_entry.text().tr);
                },
            },
            gtk::ComboBoxText {
                append_all_and_select: (
                    [
                        ColumnType::String.to_string(),
                        ColumnType::Number.to_string(),
                        ColumnType::Date.to_string(),
                    ],
                    Some(0),
                ),
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
}

#[derive(Debug)]
pub(crate) enum TypeMsg {
    Add,
    AddAbove(DynamicIndex),
    Delete(DynamicIndex),
    MoveUp(DynamicIndex),
    MoveDown(DynamicIndex),
}

trait RestoreMoveValid {
    fn restore_move_valid(&mut self);
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
}

#[derive(Debug)]
pub(crate) enum ValidityMsg {
    ValidityChanged(Validity),
}

#[relm4::component(pub(crate))]
impl SimpleComponent for Type {
    type Input = TypeMsg;
    type Output = ValidityMsg;
    type Init = ();
    type Widgets = TypeWidgets;

    view! {
        #[root]
        gtk::Box{
            set_orientation: gtk::Orientation::Vertical,
            #[name(row_box)]
            gtk::Box{
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
        _init: Self::Init,
        root: &Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let widgets = view_output!();

        let model = Type {
            ty: FactoryVecDeque::new(widgets.row_box.clone(), &sender.input),
        };

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        let mut types = self.ty.guard();
        match message {
            TypeMsg::Add => {
                types.push_back(());
                types.restore_move_valid();
            }
            TypeMsg::AddAbove(idx) => {
                let idx = idx.current_index();
                types.insert(idx, ());
                types.restore_move_valid();
            }
            TypeMsg::Delete(idx) => {
                let idx = idx.current_index();
                types.remove(idx);
                types.restore_move_valid();
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
        }
    }
}
