extern crate itertools;
extern crate ascii;

mod parser;
mod ast;
mod tokenizer;
mod smpl_type;

use std::ops::Range;

pub type Span = Range<usize>;
