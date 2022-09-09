use crate::analysis::{ColumnType, RowData};
use gtk::glib::{self, Object, Type, Value};
use gtk::prelude::*;
use gtk::subclass::prelude::ObjectSubclassIsExt;
use num_enum::{FromPrimitive, IntoPrimitive};
use relm4::gtk;

glib::wrapper! {
    pub(crate) struct TypeDef(ObjectSubclass<imp::TypeDefImp>)
        @extends gtk::Grid, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

pub(crate) const VALIDITY: &str = "validity";

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub(crate) enum RowChange {
    Add,
    Delete,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, IntoPrimitive, FromPrimitive)]
#[repr(i32)]
pub(crate) enum Validity {
    NoRows,
    NotFilled,
    Duplicates,
    #[num_enum(default)]
    Valid,
}

impl ToValue for RowChange {
    fn to_value(&self) -> Value {
        let b: bool = (*self).into();
        b.to_value()
    }

    fn value_type(&self) -> Type {
        Type::BOOL
    }
}

impl From<bool> for RowChange {
    fn from(b: bool) -> Self {
        match b {
            true => RowChange::Add,
            false => RowChange::Delete,
        }
    }
}

impl From<RowChange> for bool {
    fn from(r: RowChange) -> Self {
        match r {
            RowChange::Add => true,
            RowChange::Delete => false,
        }
    }
}

impl TypeDef {
    pub(crate) fn new() -> Self {
        Object::new(&[]).expect("Failed to create `TypeDef`.")
    }

    pub(crate) fn replicate(&self, query: &RowData) {
        let imp = self.imp();
        imp.replicate(self, &query.0);
    }

    pub(crate) fn row_data(&self) -> RowData {
        RowData(
            self.imp()
                .row_iter()
                .into_iter()
                .map(|idx| idx as i32)
                .map(|idx| {
                    let name_entry = self
                        .child_at(0, idx)
                        .unwrap()
                        .downcast::<gtk::Entry>()
                        .unwrap()
                        .text()
                        .trim()
                        .to_string();
                    let ty = self
                        .child_at(1, idx)
                        .unwrap()
                        .downcast::<gtk::ComboBoxText>()
                        .unwrap()
                        .active()
                        .map(ColumnType::try_from)
                        .unwrap()
                        .unwrap();
                    (name_entry, ty)
                })
                .collect(),
        )
    }

    fn add_row(&self, idx: u32) {
        let imp = self.imp();
        imp.add_row(self, idx);
    }

    fn delete_row(&self, idx: u32) {
        let imp = self.imp();
        imp.delete_row(self, idx);
    }

    fn move_row_up(&self, idx: u32) {
        let imp = self.imp();
        imp.move_row_up(self, idx);
    }

    fn move_row_down(&self, idx: u32) {
        let imp = self.imp();
        imp.move_row_down(self, idx);
    }
}

impl Default for TypeDef {
    fn default() -> Self {
        Self::new()
    }
}

mod imp {
    use super::VALIDITY;
    use crate::analysis::ColumnType;
    use crate::AppendAll;
    use relm4::gtk::prelude::*;
    use relm4::gtk::subclass::prelude::*;
    use relm4::gtk::{
        self,
        glib::{self, ParamSpec, Value},
    };
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use tap::Tap;

    struct RowState {
        idx: Rc<Cell<u32>>,
        up_valid: bool,
        down_valid: bool,
    }

    impl RowState {
        fn new(idx: u32) -> Self {
            RowState {
                idx: Rc::new(Cell::new(idx)),
                up_valid: true,
                down_valid: true,
            }
        }

        fn idx(&self) -> Rc<Cell<u32>> {
            Rc::clone(&self.idx)
        }

        fn get_idx(&self) -> u32 {
            self.idx.get()
        }

        fn set_idx(&self, idx: u32) {
            self.idx.set(idx);
        }

        fn disable_up_and_down(&mut self) {
            self.up_valid = false;
            self.down_valid = false;
        }

        fn inc_idx(&self) {
            self.idx.set(self.idx.get() + 1);
        }

        fn dec_idx(&self) {
            self.idx.set(self.idx.get() - 1);
        }
    }

    impl Clone for RowState {
        fn clone(&self) -> Self {
            RowState {
                idx: Rc::clone(&self.idx),
                up_valid: true,
                down_valid: true,
            }
        }
    }

    trait RestoreMoveValid {
        fn restore_move_valid(&mut self);
    }

    impl RestoreMoveValid for Vec<RowState> {
        fn restore_move_valid(&mut self) {
            for &(idx, up, down) in match self.len() {
                0 => return,
                1 => &[] as &'static [(i32, bool, bool)],
                2 => &[(0, false, false)],
                3 => &[(0, false, true), (1, true, false)],
                4 => &[(0, false, true), (1, true, true), (2, true, false)],
                _ => &[
                    (0, false, true),
                    (1, true, true),
                    (-3, true, true),
                    (-2, true, false),
                ],
            } {
                let idx = if idx < 0 {
                    self.len() as i32 + idx
                } else {
                    idx
                } as usize;
                self[idx].up_valid = up;
                self[idx].down_valid = down;
            }
        }
    }

    #[derive(Default)]
    pub(crate) struct TypeDefImp(RefCell<Vec<RowState>>);

    #[gtk::glib::object_subclass]
    impl ObjectSubclass for TypeDefImp {
        const NAME: &'static str = "SQLBonTypeDef";
        type Type = super::TypeDef;
        type ParentType = gtk::Grid;
    }

    impl TypeDefImp {
        pub(super) fn add_row(&self, obj: &super::TypeDef, idx: u32) {
            let row_state = RowState::new(idx);
            let index = idx as i32;
            let mut indices = self.0.borrow_mut();
            TypeDefImp::add_row_widgets(obj, index, &row_state.idx, None);

            {
                let idx = idx as usize;
                indices[idx..].iter().for_each(RowState::inc_idx);
                indices.insert(idx, row_state);
                indices.restore_move_valid();
            }

            for index in [index - 1, index, index + 1] {
                if index >= 0 && index < indices.len() as i32 - 1 {
                    let row_state = &indices[index as usize];
                    obj.child_at(4, index)
                        .unwrap()
                        .set_sensitive(row_state.up_valid);
                    obj.child_at(5, index)
                        .unwrap()
                        .set_sensitive(row_state.down_valid);
                }
            }
        }

        pub(super) fn delete_row(&self, obj: &super::TypeDef, idx: u32) {
            let mut indices = self.0.borrow_mut();
            let index = idx as i32;
            obj.remove_row(index);

            {
                let idx = idx as usize;
                indices[idx + 1..].iter().for_each(RowState::dec_idx);
                indices.remove(idx);
                indices.restore_move_valid();
            }

            for index in [index - 1, index] {
                if index >= 0 && index < indices.len() as i32 - 1 {
                    let row_state = &indices[index as usize];
                    obj.child_at(4, index)
                        .unwrap()
                        .set_sensitive(row_state.up_valid);
                    obj.child_at(5, index)
                        .unwrap()
                        .set_sensitive(row_state.down_valid);
                }
            }
        }

        pub(super) fn move_row_up(&self, obj: &super::TypeDef, idx: u32) {
            let idx = idx as i32;
            if idx > 0 {
                let current = obj
                    .child_at(0, idx)
                    .unwrap()
                    .downcast::<gtk::Entry>()
                    .unwrap();
                let upper = obj
                    .child_at(0, idx - 1)
                    .unwrap()
                    .downcast::<gtk::Entry>()
                    .unwrap();
                let tmp = current.text();
                current.set_text(&upper.text());
                upper.set_text(&tmp);
                let current = obj
                    .child_at(1, idx)
                    .unwrap()
                    .downcast::<gtk::ComboBoxText>()
                    .unwrap();
                let upper = obj
                    .child_at(1, idx - 1)
                    .unwrap()
                    .downcast::<gtk::ComboBoxText>()
                    .unwrap();
                let tmp = current.active();
                current.set_active(upper.active());
                upper.set_active(tmp);
            }
        }

        pub(super) fn move_row_down(&self, obj: &super::TypeDef, idx: u32) {
            let idx = idx as i32;
            // the last row is the lonely new button, which we also need to avoid.
            //                 v
            if (idx as usize + 2) < self.0.borrow().len() {
                let current = obj
                    .child_at(0, idx)
                    .unwrap()
                    .downcast::<gtk::Entry>()
                    .unwrap();
                let lower = obj
                    .child_at(0, idx + 1)
                    .unwrap()
                    .downcast::<gtk::Entry>()
                    .unwrap();
                let tmp = current.text();
                current.set_text(&lower.text());
                lower.set_text(&tmp);
                let current = obj
                    .child_at(1, idx)
                    .unwrap()
                    .downcast::<gtk::ComboBoxText>()
                    .unwrap();
                let lower = obj
                    .child_at(1, idx + 1)
                    .unwrap()
                    .downcast::<gtk::ComboBoxText>()
                    .unwrap();
                let tmp = current.active();
                current.set_active(lower.active());
                lower.set_active(tmp);
            }
        }

        pub(super) fn replicate(&self, obj: &super::TypeDef, query: &[(String, ColumnType)]) {
            let mut indices = self.0.borrow_mut();
            let len = indices.len();
            for _ in 0..(len - 1) {
                indices.remove(0);
            }

            {
                let len = query.len() as u32;
                indices[0].set_idx(len);
                for idx in (0..len).rev() {
                    indices.insert(0, RowState::new(idx));
                }
                indices.restore_move_valid();
            }

            //--------------------------------------------------------------------------------------
            for _ in 0..(len - 1) {
                obj.remove_row(0);
            }
            for ((name, ty), row_state) in query.iter().zip(indices.iter()) {
                let (move_row_up, move_row_down) = TypeDefImp::add_row_widgets(
                    obj,
                    row_state.get_idx().try_into().unwrap(),
                    &row_state.idx,
                    Some((name.as_str(), *ty)),
                );
                move_row_up.set_sensitive(row_state.up_valid);
                move_row_down.set_sensitive(row_state.down_valid);
            }
        }

        pub(super) fn row_iter(&self) -> Vec<u32> {
            let states = self.0.borrow();
            states[0..states.len() - 1]
                .iter()
                .map(RowState::get_idx)
                .collect()
        }

        fn add_row_widgets(
            obj: &super::TypeDef,
            index: i32,
            idx: &Rc<Cell<u32>>,
            init: Option<(&str, ColumnType)>,
        ) -> (gtk::Button, gtk::Button) {
            let ty = gtk::ComboBoxText::new();
            ty.append_all_and_select(
                [
                    ColumnType::String.to_string(),
                    ColumnType::Number.to_string(),
                    ColumnType::Date.to_string(),
                ],
                Some(0),
            );
            let name = gtk::Entry::new();
            let new_row = gtk::Button::with_label("new");
            let delete_row = gtk::Button::with_label("delete");
            let move_row_up = gtk::Button::with_label("up");
            let move_row_down = gtk::Button::with_label("down");
            {
                if let Some((n, t)) = init {
                    name.set_text(n);
                    ty.set_active(Some(t.into()));
                }
            }
            {
                let idx = Rc::clone(idx);
                let obj = obj.clone();
                new_row.connect_clicked(move |_| {
                    obj.add_row(idx.get());
                });
            }
            {
                let idx = Rc::clone(idx);
                let obj = obj.clone();
                delete_row.connect_clicked(move |_| {
                    obj.delete_row(idx.get());
                });
            }
            {
                let idx = Rc::clone(idx);
                let obj = obj.clone();
                move_row_up.connect_clicked(move |_| {
                    obj.move_row_up(idx.get());
                });
            }
            {
                let idx = Rc::clone(idx);
                let obj = obj.clone();
                move_row_down.connect_clicked(move |_| {
                    obj.move_row_down(idx.get());
                });
            }
            obj.insert_row(index);
            obj.attach(&name, 0, index, 1, 1);
            obj.attach(&ty, 1, index, 1, 1);
            obj.attach(&new_row, 2, index, 1, 1);
            obj.attach(&delete_row, 3, index, 1, 1);
            obj.attach(&move_row_up, 4, index, 1, 1);
            obj.attach(&move_row_down, 5, index, 1, 1);
            (move_row_up, move_row_down)
        }
    }

    // Trait shared by all GObjects
    impl ObjectImpl for TypeDefImp {
        fn set_property(&self, _obj: &Self::Type, _id: usize, _value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                VALIDITY => {
                    eprintln!("{} is read only", VALIDITY);
                }
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            let row_state = RowState::new(0).tap_mut(RowState::disable_up_and_down);
            let idx = row_state.idx();
            self.0.borrow_mut().push(row_state);
            let new_button = gtk::Button::new();
            {
                let obj = obj.clone();
                new_button.set_label("new");
                new_button.connect_clicked(move |_| {
                    obj.add_row(idx.get());
                });
            }
            obj.attach(&new_button, 0, 0, 6, 1);
        }
    }

    // Trait shared by all widgets
    impl WidgetImpl for TypeDefImp {}

    // Trait shared by all grids
    impl GridImpl for TypeDefImp {}
}
