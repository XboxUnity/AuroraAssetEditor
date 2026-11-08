use std::rc::Rc;

use slint::{Model, ModelRc, VecModel};

/// A list the window repeats, in one model the window keeps. A new model would make the window build every
/// repeated element anew, which ends a press or a drag in progress on one of them; a row changed in place
/// keeps its element. A render changes only the rows whose content changed.
///
/// `Content` is what a row shows, made to be compared cheaply: images are named by a key, never compared by
/// their pixels. `Row` is the struct the window takes.
pub struct KeptRows<Content, Row: 'static> {
    model: Rc<VecModel<Row>>,
    shown: Vec<Content>,
}

impl<Content, Row: 'static> Default for KeptRows<Content, Row> {
    fn default() -> Self {
        KeptRows {
            model: Rc::new(VecModel::default()),
            shown: Vec::new(),
        }
    }
}

impl<Content: PartialEq, Row: Clone + 'static> KeptRows<Content, Row> {
    /// The model the window shows, for as long as the window lives.
    pub fn model(&self) -> ModelRc<Row> {
        ModelRc::from(Rc::clone(&self.model))
    }

    /// Shows `contents`; `make_row` builds the row for each one that differs from what the window shows.
    pub fn show(&mut self, contents: Vec<Content>, mut make_row: impl FnMut(&Content) -> Row) {
        for (index, content) in contents.iter().enumerate() {
            match self.shown.get(index) {
                Some(shown) if shown == content => {}
                Some(_) => self.model.set_row_data(index, make_row(content)),
                None => self.model.push(make_row(content)),
            }
        }
        for index in (contents.len()..self.shown.len()).rev() {
            self.model.remove(index);
        }
        self.shown = contents;
    }
}
