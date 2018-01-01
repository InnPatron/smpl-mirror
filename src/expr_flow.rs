use std::cell::Cell;

use petgraph;
use petgraph::graph;
use petgraph::visit::EdgeRef;

use semantic_ck::{FnId, TypeId, VarId};
use ast::{FnCall as AstFnCall, Ident as AstIdent, Literal, UniOp, BinOp};

pub struct ExprGraph {
    graph: graph::Graph<Node, ()>,
}

impl ExprGraph {
    pub fn new() -> ExprGraph {
        let graph = graph::Graph::new();

        ExprGraph {
            graph
        }
    }

    pub fn graph(&self) -> &graph::Graph<Node, ()> {
        &self.graph
    }
}

pub enum Node {
    BinExpr(Typed<BinOp>),
    UniOp(Typed<UniOp>),
    Literal(Typed<Literal>),
    Ident(Typed<Ident>),
    FnCall(Typed<FnCall>),
}

#[derive(Debug)]
pub struct Typed<T> where T: ::std::fmt::Debug {
    data: T,
    data_type: Cell<Option<TypeId>>
}

impl<T> Typed<T> where T: ::std::fmt::Debug {
    fn untyped(data: T) -> Typed<T> {
        Typed {
            data: data,
            data_type: Cell::new(None) 
        }
    }

    pub fn set_type(&self, t: TypeId) {
        // TODO: Handle type override
        if self.data_type.get().is_some() {
            panic!("Attempting to overwrite the type of this node ({:?})", self);
        } else {
            self.data_type.set(Some(t));
        }
    }

    pub fn get_type(&self) -> Option<TypeId> {
        self.data_type.get()
    }

    pub fn data(&self) -> &T {
        &self.data
    }
}

#[derive(Debug)]
pub struct Ident {
    ident: AstIdent,
    var_id: Cell<Option<VarId>>,
}

impl Ident {
    fn new(ident: AstIdent) -> Ident {
        Ident {
            ident: ident,
            var_id: Cell::new(None),
        }
    }

    pub fn set_id(&self, id: VarId) {
        if self.var_id.get().is_some() {
            panic!("Attempting to overwrite the VarId ({}) of the Ident {:?} with VarId {}", self.var_id.get().unwrap().0, self.ident, id.0);
        } else {
            self.var_id.set(Some(id));
        }
    }

    pub fn get_id(&self) -> Option<VarId> {
        self.var_id.get()
    }
}

#[derive(Debug)]
pub struct FnCall {
    fn_call: AstFnCall,
    fn_id: Cell<Option<FnId>>,
}

impl FnCall {
    fn new(call: AstFnCall) -> FnCall {
        FnCall {
            fn_call: call,
            fn_id: Cell::new(None),
        }
    }

    pub fn set_id(&self, id: FnId) {
        if self.fn_id.get().is_some() {
            panic!("Attempting to overwrite the FnId ({}) of the FnCall {:?} with FnId {}", self.fn_id.get().unwrap().0, self.fn_call, id.0);
        } else {
            self.fn_id.set(Some(id));
        }
    }

    pub fn get_id(&self) -> Option<FnId> {
        self.fn_id.get()
    }
}
