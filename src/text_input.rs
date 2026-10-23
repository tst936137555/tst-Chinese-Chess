//! Character-indexed single-line editing for persisted names.
#[derive(Clone, Debug)]
pub struct NameEdit {
    pub text: String,
    pub cursor: usize,
    pub anchor: Option<usize>,
}
impl From<String> for NameEdit {
    fn from(text: String) -> Self {
        let mut edit = Self {
            text: String::new(),
            cursor: 0,
            anchor: None,
        };
        edit.insert(&text);
        edit
    }
}
impl From<&str> for NameEdit {
    fn from(text: &str) -> Self {
        text.to_owned().into()
    }
}
impl NameEdit {
    pub fn selection(&self) -> std::ops::Range<usize> {
        let anchor = self.anchor.unwrap_or(self.cursor);
        anchor.min(self.cursor)..anchor.max(self.cursor)
    }
    fn byte(&self, index: usize) -> usize {
        self.text
            .char_indices()
            .nth(index)
            .map_or(self.text.len(), |(i, _)| i)
    }
    fn erase_selection(&mut self) -> bool {
        let range = self.selection();
        self.anchor = None;
        if range.is_empty() {
            return false;
        }
        self.text
            .replace_range(self.byte(range.start)..self.byte(range.end), "");
        self.cursor = range.start;
        true
    }
    pub fn insert(&mut self, value: &str) {
        self.erase_selection();
        let remaining = 40usize.saturating_sub(self.text.chars().count());
        let clean: String = value
            .chars()
            .filter(|c| !c.is_control())
            .take(remaining)
            .collect();
        let at = self.byte(self.cursor);
        self.text.insert_str(at, &clean);
        self.cursor += clean.chars().count();
    }
    pub fn backspace(&mut self) {
        if !self.erase_selection() && self.cursor > 0 {
            let end = self.byte(self.cursor);
            self.cursor -= 1;
            self.text.replace_range(self.byte(self.cursor)..end, "");
        }
    }
    pub fn delete(&mut self) {
        if !self.erase_selection() && self.cursor < self.text.chars().count() {
            self.text
                .replace_range(self.byte(self.cursor)..self.byte(self.cursor + 1), "");
        }
    }
    pub fn move_to(&mut self, cursor: usize, selecting: bool) {
        if selecting {
            self.anchor.get_or_insert(self.cursor);
        } else {
            self.anchor = None;
        }
        self.cursor = cursor.min(self.text.chars().count());
    }
    pub fn select_all(&mut self) {
        self.anchor = Some(0);
        self.cursor = self.text.chars().count();
    }
    pub fn selected(&self) -> String {
        let range = self.selection();
        self.text
            .chars()
            .skip(range.start)
            .take(range.len())
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_selection_insertion_deletion_and_limit() {
        let mut edit = NameEdit::from("开局练习");
        edit.move_to(2, false);
        edit.insert("中文");
        assert_eq!(edit.text, "开局中文练习");
        edit.backspace();
        edit.delete();
        assert_eq!(edit.text, "开局中习");
        edit.select_all();
        assert_eq!(edit.selected(), "开局中习");
        edit.insert("残局\n复盘");
        assert_eq!(edit.text, "残局复盘");
        edit.insert(&"棋".repeat(50));
        assert_eq!(edit.text.chars().count(), 40);
        assert_eq!(edit.cursor, 40);
    }
}
