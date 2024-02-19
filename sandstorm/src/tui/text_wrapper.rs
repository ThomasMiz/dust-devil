use std::{borrow::Cow, ops::Deref};

use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::utils::setnext_iter::SetNextIter;

#[derive(Clone)]
pub enum StaticString {
    Static(&'static str),
    Owned(String),
}

impl StaticString {
    pub fn trim_starting_whitespace(&mut self) {
        match self {
            Self::Static(s) => *s = s.trim_start(),
            Self::Owned(s) => {
                let final_length = s.trim_start().len();
                let bytes_to_trim = s.len() - final_length;
                if bytes_to_trim != 0 {
                    unsafe {
                        let bytes = s.as_mut_vec();
                        bytes.copy_within(bytes_to_trim.., 0);
                        bytes.set_len(final_length);
                    }
                }
            }
        }
    }

    pub fn trim_trailing_whitespace(&mut self) {
        match self {
            Self::Static(s) => *s = s.trim_end(),
            Self::Owned(s) => {
                let final_length = s.trim_end().len();
                unsafe {
                    s.as_mut_vec().set_len(final_length);
                }
            }
        }
    }

    pub fn split_at(self, index: usize, trim_whitespaces: bool) -> (StaticString, StaticString) {
        let (mut left, mut right) = match self {
            StaticString::Static(s) => (StaticString::Static(&s[0..index]), StaticString::Static(&s[index..])),
            StaticString::Owned(mut s) => {
                let s2 = String::from(&s[index..]);
                s.truncate(index);
                (StaticString::Owned(s), StaticString::Owned(s2))
            }
        };

        if trim_whitespaces {
            left.trim_trailing_whitespace();
            right.trim_starting_whitespace();
        }

        (left, right)
    }
}

impl Deref for StaticString {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Static(s) => s,
            Self::Owned(s) => s,
        }
    }
}

impl Default for StaticString {
    fn default() -> Self {
        StaticString::Static("")
    }
}

impl From<String> for StaticString {
    fn from(value: String) -> Self {
        StaticString::Owned(value)
    }
}

impl From<&'static str> for StaticString {
    fn from(value: &'static str) -> Self {
        Self::Static(value)
    }
}

impl From<StaticString> for Cow<'static, str> {
    fn from(value: StaticString) -> Self {
        match value {
            StaticString::Static(s) => s.into(),
            StaticString::Owned(s) => s.into(),
        }
    }
}

pub fn wrap_lines_by_chars<F, T>(wrap_width: usize, text: T, mut f: F)
where
    F: FnMut(Line<'static>),
    T: DoubleEndedIterator<Item = (StaticString, Style)>,
{
    let mut line_vec = Vec::new();
    let mut current_line_length = 0;
    let mut iter = SetNextIter::new(text);

    'outer: while let Some((text, style)) = iter.next() {
        let text_chars_iter = text.char_indices();

        for (next_char_i, _next_char) in text_chars_iter {
            current_line_length += 1;
            if current_line_length > wrap_width {
                let (left, right) = text.split_at(next_char_i, true);
                line_vec.push(Span::styled(left, style));
                iter.set_next((right, style));

                f(Line::from(line_vec));
                line_vec = Vec::new();
                current_line_length = 0;

                continue 'outer;
            }
        }

        line_vec.push(Span::styled(text, style));

        if current_line_length == wrap_width {
            f(Line::from(line_vec));
            line_vec = Vec::new();
            current_line_length = 0;
        }
    }

    if !line_vec.is_empty() {
        f(Line::from(line_vec));
    }
}

pub struct WrapTextIter<'a> {
    remaining_text: &'a str,
    remaining_text_index: usize,
    wrap_width: usize,
}

impl<'a> WrapTextIter<'a> {
    pub fn new(text: &'a str, wrap_width: usize) -> Self {
        Self {
            remaining_text: text.trim(),
            remaining_text_index: 0,
            wrap_width,
        }
    }
}

pub struct WrapTextIterItem {
    pub index_start: usize,
    pub len_bytes: usize,
    pub len_chars: usize,
}

impl WrapTextIterItem {
    pub fn get_substr<'a>(&self, s: &'a str) -> &'a str {
        &s[self.index_start..(self.index_start + self.len_bytes)]
    }
}

impl<'a> Iterator for WrapTextIter<'a> {
    type Item = WrapTextIterItem;

    fn next(&mut self) -> Option<Self::Item> {
        let mut chars_iter = self.remaining_text.char_indices();

        chars_iter.next()?;

        let mut char_count = 1;
        let mut split_at_index = 0;
        let mut char_count_at_split_index = 0;
        loop {
            match chars_iter.next() {
                Some((index, c)) => {
                    if c.is_whitespace() {
                        split_at_index = index;
                        char_count_at_split_index = char_count;
                    }

                    char_count += 1;
                    if char_count >= self.wrap_width {
                        if split_at_index == 0 || !chars_iter.next().is_some_and(|(_i, c)| !c.is_whitespace()) {
                            split_at_index = index + c.len_utf8();
                            char_count_at_split_index = char_count;
                        }
                        break;
                    }
                }
                None => {
                    split_at_index = self.remaining_text.len();
                    char_count_at_split_index = char_count;
                    break;
                }
            }
        }

        let retval = WrapTextIterItem {
            index_start: self.remaining_text_index,
            len_bytes: split_at_index,
            len_chars: char_count_at_split_index,
        };

        let remaining_trimmed = self.remaining_text[split_at_index..].trim_start();
        self.remaining_text_index += self.remaining_text.len() - remaining_trimmed.len();
        self.remaining_text = remaining_trimmed;

        Some(retval)
    }
}
