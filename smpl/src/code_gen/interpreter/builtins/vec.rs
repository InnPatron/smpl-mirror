use failure::Error;

use crate::{no_args, exact_args};
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;

use strfmt::strfmt;

use crate::ast::Module;
use crate::parser::parse_module;

use crate::code_gen::interpreter::*;

pub const MOD_VEC: &'static str = "vec_{item_type}";
pub const VEC_NEW: &'static str = "new";
pub const VEC_LEN: &'static str = "len";
pub const VEC_CONTAINS: &'static str = "contains";
pub const VEC_PUSH: &'static str = "push";
pub const VEC_INSERT: &'static str = "insert";
pub const VEC_GET: &'static str = "get";
pub const VEC_REMOVE: &'static str = "remove";

pub const VEC_DATA_KEY: &'static str = "__DATA";
pub const VEC_LEN_KEY: &'static str = "__LEN";

const VEC_FMT_ITEM_TYPE: &'static str = "item_type";
const VEC_FMT_ITEM_TYPE_MOD: &'static str = "item_type_mod";
const VEC_FMT_ITEM_USE: &'static str = "item_mod_use";

const VEC_DECLARATION: &'static str = include_str!("vec.smpl");

// Returns the correct include function for a given type
pub fn include(item_type_mod: Option<&str>, item_type: &str) -> 
    Box<dyn Fn(&mut Vec<Module>) -> Result<(), crate::err::Err>>{

    let item_type = item_type.to_string();

    let item_mod_use = match item_type_mod {
        Some(str) => format!("use {};", str),
        None => "".to_string(),
    };

    let item_type_mod = match item_type_mod {
        Some(str) => format!("{}::", str),
        None => "".to_string(),
    };
    Box::new( move |modules| {
        let mut vars = HashMap::new();
        vars.insert(VEC_FMT_ITEM_TYPE.to_string(), &item_type);
        vars.insert(VEC_FMT_ITEM_TYPE_MOD.to_string(), &item_type_mod);
        vars.insert(VEC_FMT_ITEM_USE.to_string(), &item_mod_use);

        let decl = strfmt(&VEC_DECLARATION, &vars).unwrap();
        modules.push(parse_module(&decl).unwrap());

        Ok(())
    })
}

pub fn add(vm: &mut dyn BuiltinMap, item_type: &str) {
    let mut vars = HashMap::new();
    vars.insert(VEC_FMT_ITEM_TYPE.to_string(), item_type);

    let mod_name = strfmt(MOD_VEC, &vars).unwrap();

    vm.insert_builtin(&mod_name, VEC_NEW, new)
        .unwrap();
    vm.insert_builtin(&mod_name, VEC_LEN, len)
        .unwrap();
    vm.insert_builtin(&mod_name, VEC_CONTAINS, contains)
        .unwrap();
    vm.insert_builtin(&mod_name, VEC_PUSH, push)
        .unwrap();
    vm.insert_builtin(&mod_name, VEC_INSERT, insert)
        .unwrap();
    vm.insert_builtin(&mod_name, VEC_GET, get)
        .unwrap();
    vm.insert_builtin(&mod_name, VEC_REMOVE, remove)
        .unwrap();
}

#[derive(Fail, Debug)]
pub enum VecError {
    #[fail(display = "Index '{}' out of range ('{}')", _0, _1)]
    IndexOutOfRange(i64, usize)
}

fn new(args: Option<Vec<Value>>) -> Result<Value, Error> {
    let _args: Option<Vec<Value>> = no_args!(args)?;

    let mut vec = Struct::new();
    vec.set_field(VEC_DATA_KEY.to_string(), Value::Array(Vec::new()));
    vec.set_field(VEC_LEN_KEY.to_string(), Value::Int(0));

    Ok(Value::Struct(vec))
}

fn len(args: Option<Vec<Value>>) -> Result<Value, Error> {
    let mut args = exact_args!(1, args)?;
    let vec_struct = args.pop().unwrap();
    let vec_struct = irmatch!(vec_struct; Value::Struct(s) => s);

    let length = vec_struct.get_field(VEC_LEN_KEY).unwrap();

    Ok(length)
}

fn contains(args: Option<Vec<Value>>) -> Result<Value, Error> {
    let mut args = exact_args!(2, args)?;

    let to_search = args.pop().unwrap();

    let vec_struct = args.pop().unwrap();
    let vec_struct = irmatch!(vec_struct; Value::Struct(s) => s);

    let data = vec_struct.ref_field(VEC_DATA_KEY).unwrap();

    let borrow = data.borrow();
    let data = irmatch!(*borrow; Value::Array(ref a) => a);

    for element in data {
        let element = element.borrow();
        if *element == to_search {
            return Ok(Value::Bool(true));
        }
    }

    Ok(Value::Bool(false))
}

fn insert(args: Option<Vec<Value>>) -> Result<Value, Error> {
    let mut args = exact_args!(3, args)?;

    let to_insert = args.pop().unwrap();
    let index = args.pop().unwrap();
    let index = irmatch!(index; Value::Int(i) => i);

    let vec_struct = args.pop().unwrap();
    let vec_struct = irmatch!(vec_struct; Value::Struct(s) => s);

    {
        let data = vec_struct.ref_field(VEC_DATA_KEY).unwrap();

        let mut borrow = data.borrow_mut();
        let data = irmatch!(*borrow; Value::Array(ref mut a) => a);
        data.insert(index as usize, Rc::new(RefCell::new(to_insert)));
    }

    {
        let len = vec_struct.ref_field(VEC_LEN_KEY).unwrap();
        let mut borrow = len.borrow_mut();
        let len = irmatch!(*borrow; Value::Int(ref mut i) => i);
        *len += 1;
    }

    Ok(Value::Struct(vec_struct))
}

fn push(args: Option<Vec<Value>>) -> Result<Value, Error> {
    let mut args = exact_args!(2, args)?;

    let to_insert = args.pop().unwrap();

    let vec_struct = args.pop().unwrap();
    let vec_struct = irmatch!(vec_struct; Value::Struct(s) => s);

    {
        let data = vec_struct.ref_field(VEC_DATA_KEY).unwrap();

        let mut borrow = data.borrow_mut();
        let data = irmatch!(*borrow; Value::Array(ref mut a) => a);
        data.push(Rc::new(RefCell::new(to_insert)));
    }

    {
        let len = vec_struct.ref_field(VEC_LEN_KEY).unwrap();
        let mut borrow = len.borrow_mut();
        let len = irmatch!(*borrow; Value::Int(ref mut i) => i);
        *len += 1;
    }

    Ok(Value::Struct(vec_struct))
}

fn get(args: Option<Vec<Value>>) -> Result<Value, Error> {
    let mut args = exact_args!(2, args)?;

    let index = args.pop().unwrap();
    let smpl_index = irmatch!(index; Value::Int(i) => i) as i64;

    let vec_struct = args.pop().unwrap();
    let vec_struct = irmatch!(vec_struct; Value::Struct(s) => s);

    let data = vec_struct.ref_field(VEC_DATA_KEY).unwrap();

    let borrow = data.borrow();
    let data = irmatch!(*borrow; Value::Array(ref a) => a);

    let index: usize = if smpl_index < 0 {
        return Err(VecError::IndexOutOfRange(smpl_index, data.len()))?;
    } else {
        smpl_index as usize
    };

    let item = data
        .get(index)
        .map(|rc| (*rc.borrow()).clone())
        .ok_or(VecError::IndexOutOfRange(smpl_index, data.len()))?;

    Ok(item)
}

fn remove(args: Option<Vec<Value>>) -> Result<Value, Error> {
    let mut args = exact_args!(2, args)?;

    let index = args.pop().unwrap();
    let smpl_index = irmatch!(index; Value::Int(i) => i) as i64;

    let vec_struct = args.pop().unwrap();
    let vec_struct = irmatch!(vec_struct; Value::Struct(s) => s);

    {
        let data = vec_struct.ref_field(VEC_DATA_KEY).unwrap();
        let mut borrow = data.borrow_mut();
        let data = irmatch!(*borrow; Value::Array(ref mut a) => a);

        let index: usize = if smpl_index < 0 {
            return Err(VecError::IndexOutOfRange(smpl_index, data.len()))?;
        } else {
            smpl_index as usize
        };

        if index >= data.len() {
            return Err(VecError::IndexOutOfRange(smpl_index, data.len()))?;
        } else {
            data.remove(index);
        }
    }

    {
        let len = vec_struct.ref_field(VEC_LEN_KEY).unwrap();
        let mut borrow = len.borrow_mut();
        let len = irmatch!(*borrow; Value::Int(ref mut i) => i);
        *len -= 1;
    }

    Ok(Value::Struct(vec_struct))
}

#[cfg(test)]
#[cfg_attr(rustfmt, rustfmt_skip)]
mod tests {

use super::*;

#[test]
fn interpreter_vec_new() {
    let mod1 =
"
mod mod1;
use vec_int;

fn vec_new() {
let v = vec_int::new();
}
";
    let mut modules = vec![parse_module(mod1).unwrap()];
    include(&mut modules, None, "int");

    let mut vm = AVM::new(modules).unwrap();
    add(&mut vm, "int");

    let fn_handle = vm.query_module("mod1", "vec_new").unwrap().unwrap();

    let result = vm.eval_fn_sync(fn_handle).unwrap();

    assert_eq!(Value::Unit, result);
}

#[test]
fn interpreter_vec_push() {
    let mod1 =
"
mod mod1;
use vec_int;

fn test() -> int {
let v = vec_int::new();
v = vec_int::push(v, 123);
v = vec_int::push(v, 456);

return vec_int::len(v);
}
";
    let mut modules = vec![parse_module(mod1).unwrap()];
    include(&mut modules, None, "int");

    let mut vm = AVM::new(modules).unwrap();
    add(&mut vm, "int");

    let fn_handle = vm.query_module("mod1", "test").unwrap().unwrap();

    let result = vm.eval_fn_sync(fn_handle).unwrap();

    assert_eq!(Value::Int(2), result);
}

#[test]
fn interpreter_vec_get() {
    let mod1 =
"
mod mod1;
use vec_int;

fn test() -> int {
let v = vec_int::new();
v = vec_int::push(v, 123);
v = vec_int::push(v, 456);

let a = vec_int::get(v, 0);
let b = vec_int::get(v, 1);

return a * b;
}
";
    let mut modules = vec![parse_module(mod1).unwrap()];
    include(&mut modules, None, "int");

    let mut vm = AVM::new(modules).unwrap();
    add(&mut vm, "int");

    let fn_handle = vm.query_module("mod1", "test").unwrap().unwrap();

    let result = vm.eval_fn_sync(fn_handle).unwrap();

    assert_eq!(Value::Int(123 * 456), result);
}

#[test]
fn interpreter_vec_remove() {
    let mod1 =
"
mod mod1;
use vec_int;

fn test() -> int {
let v = vec_int::new();
v = vec_int::push(v, 123);
v = vec_int::push(v, 456);
v = vec_int::push(v, 789);

v = vec_int::remove(v, 1);

return vec_int::get(v, 1);
}
";
    let mut modules = vec![parse_module(mod1).unwrap()];
    include(&mut modules, None, "int");

    let mut vm = AVM::new(modules).unwrap();
    add(&mut vm, "int");

    let fn_handle = vm.query_module("mod1", "test").unwrap().unwrap();

    let result = vm.eval_fn_sync(fn_handle).unwrap();

    assert_eq!(Value::Int(789), result);
}

#[test]
fn interpreter_vec_insert() {
    let mod1 =
"
mod mod1;
use vec_int;

fn test() -> int {
let v = vec_int::new();
v = vec_int::push(v, 123);
v = vec_int::push(v, 456);

v = vec_int::insert(v, 0, 1337);

let a = vec_int::get(v, 0);

return a;
}
";
    let mut modules = vec![parse_module(mod1).unwrap()];
    include(&mut modules, None, "int");

    let mut vm = AVM::new(modules).unwrap();
    add(&mut vm, "int");

    let fn_handle = vm.query_module("mod1", "test").unwrap().unwrap();

    let result = vm.eval_fn_sync(fn_handle).unwrap();

    assert_eq!(Value::Int(1337), result);
}

#[test]
fn interpreter_vec_contains() {
    let mod1 =
"
mod mod1;
use vec_int;

fn test() -> bool {
let v = vec_int::new();
v = vec_int::push(v, 1);
v = vec_int::push(v, 2);
v = vec_int::push(v, 3);
v = vec_int::push(v, 4);
v = vec_int::push(v, 5);
v = vec_int::push(v, 6);
v = vec_int::push(v, 7);

return vec_int::contains(v, 5);
}

fn test2() -> bool {
let v = vec_int::new();
v = vec_int::push(v, 1);
v = vec_int::push(v, 2);
v = vec_int::push(v, 3);
v = vec_int::push(v, 4);
v = vec_int::push(v, 5);
v = vec_int::push(v, 6);
v = vec_int::push(v, 7);

return vec_int::contains(v, 20);
}
";
    let mut modules = vec![parse_module(mod1).unwrap()];
    include(&mut modules, None, "int");

    let mut vm = AVM::new(modules).unwrap();
    add(&mut vm, "int");

    let fn_handle = vm.query_module("mod1", "test").unwrap().unwrap();

    let result = vm.eval_fn_sync(fn_handle).unwrap();

    assert_eq!(Value::Bool(true), result);

    let fn_handle = vm.query_module("mod1", "test2").unwrap().unwrap();
    let result = vm.eval_fn_sync(fn_handle).unwrap();

    assert_eq!(Value::Bool(false), result);
}
}
