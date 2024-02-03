use std::borrow::Cow;

use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::utils::setnext_iter::SetNextIter;

pub enum StaticString {
    Static(&'static str),
    Owned(String),
}

impl StaticString {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Static(s) => s,
            Self::Owned(s) => s,
        }
    }

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
        let text_chars_iter = text.as_str().char_indices();

        for (next_char_i, _next_char) in text_chars_iter {
            current_line_length += 1;
            if current_line_length > wrap_width {
                let (left, right) = text.split_at(next_char_i, true);
                line_vec.push(Span::styled(left, style));
                iter.set_next((right, style));

                f(Line::from(line_vec));
                line_vec = Vec::new();
                current_line_length -= wrap_width;

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
