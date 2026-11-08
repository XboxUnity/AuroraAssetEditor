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

#[cfg(test)]
mod tests {
    use super::*;

    /// A screenshot strip in letters: each character is one row, `+` the Add tile.
    type Strip = KeptRows<char, char>;

    /// Shows `contents` and returns how many rows `show` built.
    fn rows_built(strip: &mut Strip, contents: &str) -> usize {
        let mut built = 0;
        strip.show(contents.chars().collect(), |content| {
            built += 1;
            *content
        });
        let shown: String = strip.model().iter().collect();
        assert_eq!(shown, contents, "the model shows what was asked");
        built
    }

    #[test]
    fn a_render_builds_only_the_rows_that_changed() {
        let mut strip = Strip::default();
        let model = strip.model();

        assert_eq!(rows_built(&mut strip, "12+"), 3, "the first render");
        assert_eq!(rows_built(&mut strip, "12+"), 0, "nothing changed");
        assert_eq!(rows_built(&mut strip, "1X+"), 1, "one row changed");
        assert_eq!(rows_built(&mut strip, "1X+4"), 1, "a row added at the end");
        assert_eq!(rows_built(&mut strip, "1X"), 0, "rows removed from the end");
        assert_eq!(
            rows_built(&mut strip, "1X+"),
            1,
            "the Add tile back at the end"
        );
        assert_eq!(
            rows_built(&mut strip, "1X3+"),
            2,
            "the Add tile moved on by one"
        );
        assert_eq!(
            rows_built(&mut strip, "1X+"),
            1,
            "the Add tile moved back by one"
        );
        assert!(strip.model() == model, "the window keeps one model");
    }
}
