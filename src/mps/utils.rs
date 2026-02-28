use std::fmt;
use std::fmt::Write as _;

use crate::ir::{Index, op::RowType};

impl fmt::Display for RowType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RowType::LessThanOrEqual => write!(f, "L"),
            RowType::Equal => write!(f, "E"),
            RowType::GreaterThanOrEqual => write!(f, "G"),
            RowType::Unconstrained => write!(f, "N"),
        }
    }
}

#[inline]
pub fn write_index_vals<'a>(buf: &'a mut String, v: &Index) -> &'a str {
    buf.clear();
    if !v.is_empty() {
        buf.push('[');
        for (i, item) in v.iter().enumerate() {
            if i > 0 {
                buf.push(',');
            }
            write!(buf, "{item}").unwrap();
        }
        buf.push(']');
    }
    buf.as_str()
}
