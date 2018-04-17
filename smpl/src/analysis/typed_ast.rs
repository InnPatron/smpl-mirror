use std::cell::{RefCell, Cell};
use std::collections::HashMap;
use std::slice::Iter;

pub use ast::BinOp;
pub use ast::UniOp;
pub use ast::Literal;
use ast;

use super::smpl_type::*;
use super::semantic_data::*;
use super::expr_flow;

#[derive(Debug, Clone, PartialEq)]
pub struct Typed<T> where T: ::std::fmt::Debug + Clone + PartialEq {
    data: T,
    data_type: Cell<Option<TypeId>>
}

impl<T> Typed<T> where T: ::std::fmt::Debug + Clone + PartialEq {

    pub fn data(&self) -> &T {
        &self.data
    }

    pub fn untyped(data: T) -> Typed<T> {
        Typed {
            data: data,
            data_type: Cell::new(None) 
        }
    }

    pub fn typed(data: T, t: TypeId) -> Typed<T> {
        Typed {
            data: data,
            data_type: Cell::new(Some(t)),
        }
    }

    pub fn set_type_id(&self, t: TypeId) {
        // TODO: Handle type override
        if self.data_type.get().is_some() {
            panic!("Attempting to overwrite the type of this node ({:?})", self);
        } else {
            self.data_type.set(Some(t));
        }
    }

    pub fn type_id(&self) -> Option<TypeId> {
        self.data_type.get()
    }
}


#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    field_access: FieldAccess,
    value: self::Expr,
}

impl Assignment {
    pub fn new(universe: &Universe, assignment: ast::Assignment) -> Assignment {
        Assignment {
            field_access: FieldAccess::new(universe, assignment.name),
            value: expr_flow::flatten(universe, assignment.value),
        }
    }

    pub fn assignee(&self) -> &FieldAccess {
        &self.field_access
    }

    pub fn value(&self) -> &self::Expr {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalVarDecl {
    var_type: ast::TypeAnnotation,
    var_name: ast::Ident,
    var_init: self::Expr,
    type_id: Cell<Option<TypeId>>,
    var_id: VarId,
}

impl LocalVarDecl {
    pub fn new(universe: &Universe, decl: ast::LocalVarDecl) -> LocalVarDecl {
        LocalVarDecl {
            var_type: decl.var_type,
            var_name: decl.var_name,
            var_init: expr_flow::flatten(universe, decl.var_init),
            type_id: Cell::new(None),
            var_id: universe.new_var_id(),
        }
    }

    pub fn type_annotation(&self) -> &ast::TypeAnnotation {
        &self.var_type
    }

    pub fn var_name(&self) -> &ast::Ident {
        &self.var_name
    }

    pub fn set_type_id(&self, id: TypeId) {
        if self.type_id.get().is_some() {
            panic!("Attempting to override {} for local variable declarration {:?}", id, self);
        } else {
            self.type_id.set(Some(id));
        }
    }

    pub fn type_id(&self) -> Option<TypeId> {
        self.type_id.get()
    }

    pub fn var_id(&self) -> VarId {
        self.var_id
    }

    pub fn init_expr(&self) -> &self::Expr {
        &self.var_init
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    map: HashMap<TmpId, Tmp>,
    execution_order: Vec<TmpId>,
}

impl Expr {

    pub fn new() -> Expr {
        Expr {
            map: HashMap::new(),
            execution_order: Vec::new(),
        }
    }

    pub fn get_tmp(&self, id: TmpId) -> &Tmp {
        self.map.get(&id).expect("Given ID should always be valid if taken from the correct Expr")
    }

    pub fn execution_order(&self) -> Iter<TmpId> {
        self.execution_order.iter()
    }

    pub fn map_tmp(&mut self, universe: &Universe, val: Value) -> TmpId {
        let tmp = Tmp {
            id: universe.new_tmp_id(),
            value: Typed {
                data: val,
                data_type: Cell::new(None),
            }
        };
        let id = tmp.id;

        if self.map.insert(id, tmp).is_some() {
            panic!("Attempting to override {}", id);
        }

        self.execution_order.push(id);

        id
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tmp {
    id: TmpId,
    value: Typed<Value>,
}

impl Tmp {
    pub fn id(&self) -> TmpId {
        self.id
    }

    pub fn value(&self) -> &Typed<Value> {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Literal(ast::Literal),
    Variable(self::Variable),
    FieldAccess(self::FieldAccess),
    FnCall(self::FnCall),
    BinExpr(ast::BinOp, Typed<TmpId>, Typed<TmpId>),
    UniExpr(ast::UniOp, Typed<TmpId>),
    StructInit(StructInit),
    ArrayInit(self::ArrayInit),
    Indexing(Indexing),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Indexing {
    pub array: Typed<TmpId>,
    pub indexer: Typed<TmpId>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrayInit {
    List(Vec<Typed<TmpId>>),
    Value(Typed<TmpId>, u64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructInit {
    struct_type_name: ast::ModulePath,
    field_init: Option<Vec<(ast::Ident, Typed<TmpId>)>>,
    struct_type: Cell<Option<TypeId>>,
    mapped_field_init: RefCell<Option<Vec<(FieldId, Typed<TmpId>)>>>,
}

impl StructInit {
    pub fn new(struct_type_name: ast::ModulePath, field_init: Option<Vec<(ast::Ident, Typed<TmpId>)>>) -> StructInit {
        StructInit {
            struct_type_name: struct_type_name,
            struct_type: Cell::new(None),
            field_init: field_init,
            mapped_field_init: RefCell::new(None),
        }
    }

    pub fn type_name(&self) -> &ast::ModulePath {
        &self.struct_type_name
    }

    pub fn set_struct_type(&self, id: TypeId) {
        if self.struct_type.get().is_some() {
            panic!("Attempting to overwrite {} of the struct init with {}", self.struct_type.get().unwrap(), id);
        } else {
            self.struct_type.set(Some(id));
        }
    }

    pub fn field_init(&self) -> Option<Vec<(FieldId, Typed<TmpId>)>> {
        self.mapped_field_init.borrow()
            .clone()
    }

    pub fn set_field_init(&self, universe: &Universe) 
        -> Result<(), Vec<ast::Ident>> {
        
        let t = universe.get_type(self.struct_type.get().unwrap());
        let t = match *t {
            SmplType::Struct(ref t) => t,
            _ => unreachable!(),
        };

        match self.field_init {
            Some(ref map) => {
                let mut result = Vec::new();
                let mut unknown_fields = Vec::new();
                for &(ref ident, ref tmp) in map.iter() {
                    match t.field_id(ident) {
                        Some(id) => {
                            result.push((id, tmp.clone()));
                        }

                        None => {
                            unknown_fields.push(ident.clone());
                        }
                    }
                }

                if unknown_fields.len() > 0 {
                    Err(unknown_fields)
                } else {
                    *self.mapped_field_init.borrow_mut() = Some(result);
                    Ok(())
                }
            }

            None => Ok(()),
        }
    }

    pub fn struct_type(&self) -> Option<TypeId> {
        self.struct_type.get()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldAccess {
    raw_path: ast::Path,
    path: self::Path,
    field_type_id: Cell<Option<TypeId>>,
}

impl FieldAccess {
    pub fn new(universe: &Universe, path: ast::Path) -> FieldAccess {
        FieldAccess {
            raw_path: path.clone(),
            path: self::Path::new(universe, path),
            field_type_id: Cell::new(None),
        }
    }

    pub fn raw_path(&self) -> &ast::Path {
        &self.raw_path
    }

    pub fn path(&self) -> &self::Path {
        &self.path
    }

    pub fn set_field_type_id(&self, id: TypeId) {
        if self.field_type_id.get().is_some() {
            panic!("Attempting to override {} for local variable declarration {:?}", id, self);
        } else {
            self.field_type_id.set(Some(id));
        }
    }

    pub fn field_type_id(&self) -> Option<TypeId> {
        self.field_type_id.get()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variable {
    ident: ast::Ident,
    var_id: Cell<Option<VarId>>,
}

impl Variable {
    pub fn new(ident: ast::Ident) -> Variable {
        Variable {
            ident: ident,
            var_id: Cell::new(None),
        }
    }

    pub fn ident(&self) -> &ast::Ident {
        &self.ident
    }

    pub fn set_id(&self, id: VarId) {
        if self.var_id.get().is_some() {
            panic!("Attempting to overwrite {} of the Ident {:?} with {}", self.var_id.get().unwrap(), self.ident, id);
        } else {
            self.var_id.set(Some(id));
        }
    }

    pub fn get_id(&self) -> Option<VarId> {
        self.var_id.get()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FnCall {
    path: ast::ModulePath,
    args: Option<Vec<Typed<TmpId>>>,
    fn_id: Cell<Option<FnId>>,
}

impl FnCall {
    pub fn new(path: ast::ModulePath, args: Option<Vec<Typed<TmpId>>>) -> FnCall {
        FnCall {
            path: path,
            args: args,
            fn_id: Cell::new(None),
        }
    }

    pub fn path(&self) -> &ast::ModulePath {
        &self.path
    }

    pub fn args(&self) -> Option<&Vec<Typed<TmpId>>> {
        self.args.as_ref()
    }

    pub fn set_id(&self, id: FnId) {
        if self.fn_id.get().is_some() {
            panic!("Attempting to overwrite {} of the FnCall {:?}", self.fn_id.get().unwrap(), self.path);
        } else {
            self.fn_id.set(Some(id));
        }
    }

    pub fn get_id(&self) -> Option<FnId> {
        self.fn_id.get()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    root_name: ast::Ident,
    root_indexing: Option<self::Expr>,
    root_var: RefCell<Option<Typed<VarId>>>,
    path: Vec<self::PathSegment>,
}

impl self::Path {
    fn new(universe: &Universe, path: ast::Path) -> self::Path {
        let mut path_iter = path.0.into_iter();
        let root = path_iter.next().unwrap();

        let (name, indexing) = match root {
            ast::PathSegment::Ident(i) => (i, None),
            ast::PathSegment::Indexing(i, e) => (i, Some(expr_flow::flatten(universe, *e))),
        };

        let path = path_iter.map(|ps| {
            match ps {
                ast::PathSegment::Ident(i) => self::PathSegment::Ident(Field::new(i)),
                ast::PathSegment::Indexing(i, e) => {
                    self::PathSegment::Indexing(Field::new(i), expr_flow::flatten(universe, *e))
                }
            }
        }).collect();

        self::Path {
            root_name: name,
            root_indexing: indexing,
            root_var: RefCell::new(None),
            path: path
        }
    }

    pub fn root_name(&self) -> &ast::Ident {
        &self.root_name
    }

    pub fn root_indexing_expr(&self) -> Option<&self::Expr> {
        self.root_indexing.as_ref()
    }

    pub fn root_var_id(&self) -> VarId {
        let r = self.root_var.borrow();

        match *r {
            Some(ref typed_var_id) => *typed_var_id.data(),
            None => panic!("No root var")
        }
    }

    pub fn root_var_type(&self) -> TypeId {
        let r = self.root_var.borrow();

        match *r {
            Some(ref typed_var_id) => typed_var_id.type_id().unwrap(),
            None => panic!("No root var")
        }
    }

    pub fn set_root_var(&self, id: VarId) {
        let mut r = self.root_var.borrow_mut();

        if r.is_some() {
            panic!("Attempting to overwrite root VarId");
        }

        *r = Some(Typed::untyped(id));
    }

    pub fn set_root_var_type(&self, id: TypeId) {
        let mut r = self.root_var.borrow_mut();

        match *r {
            Some(ref t) => t.set_type_id(id),
            None => panic!("No root var"),
        }
    }

    pub fn path(&self) -> &[self::PathSegment] {
        &self.path
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PathSegment {
    Ident(Field),
    Indexing(Field, self::Expr),
}


#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    name: ast::Ident,
    field_id: RefCell<Option<Typed<FieldId>>>
}

impl Field {

    pub fn new(name: ast::Ident) -> Field {
        Field {
            name: name,
            field_id: RefCell::new(None),
        }
    }

    pub fn name(&self) -> &ast::Ident {
        &self.name
    }

    pub fn field_id(&self) -> FieldId {
        let f = self.field_id.borrow();

        match *f {
            Some(ref typed_field_id) => *typed_field_id.data(),
            None => panic!("No field")
        }
    }

    pub fn field_type(&self) -> TypeId {
        let f = self.field_id.borrow();

        match *f {
            Some(ref typed_field_id) => typed_field_id.type_id().unwrap(),
            None => panic!("No field")
        }
    }

    pub fn set_field_id(&self, id: FieldId) {
        let mut f = self.field_id.borrow_mut();

        if f.is_some() {
            panic!("Attempting to override field id.");
        }

        *f = Some(Typed::untyped(id));
    }

    pub fn set_field_type(&self, id: TypeId) {
        let mut f = self.field_id.borrow_mut();

        match *f {
            Some(ref t) => t.set_type_id(id),
            None => panic!("No field"),
        }
    }
}
