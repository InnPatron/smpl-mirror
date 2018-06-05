extern crate ascii;
extern crate itertools;
extern crate petgraph;
extern crate lalrpop_util;
#[macro_use]
extern crate irmatch;
extern crate strfmt;

mod feature;
mod err;
#[macro_use]
mod ast_macros;
mod parser;
mod ast;
#[macro_use]
mod analysis;
mod code_gen;
mod span;


pub use self::err::Err;


pub use self::parser::parse_module;
pub use self::ast::Module;


pub use analysis::Program;
pub use analysis::check_program;


pub use self::code_gen::RustBackend;
