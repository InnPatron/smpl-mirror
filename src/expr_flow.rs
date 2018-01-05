use std::cell::Cell;
use std::collections::HashMap;
use std::slice::Iter;

use semantic_ck::{Universe, FnId, TypeId, VarId, TmpId};
use ast::{FnCall as AstFnCall, Ident as AstIdent, Literal, UniOp, BinOp, Expr as AstExpr};

pub fn flatten(universe: &Universe, e: AstExpr) -> Expr {
    let mut expr = Expr {
        map: HashMap::new(),
        execution_order: Vec::new(),
    };

    flatten_expr(universe, &mut expr, e);

    expr
}

pub fn flatten_expr(universe: &Universe, scope: &mut Expr, e: AstExpr) -> TmpId {
    match e {
        AstExpr::Bin(bin) => {
            let lhs = flatten_expr(universe, scope, *bin.lhs);
            let rhs = flatten_expr(universe, scope, *bin.rhs);
            scope.map_tmp(universe, Value::BinExpr(bin.op, 
                                                   Typed::untyped(lhs),
                                                   Typed::untyped(rhs)),
                                                   None)
        }

        AstExpr::Uni(uni) => {
            let expr = flatten_expr(universe, scope, *uni.expr);
            scope.map_tmp(universe, Value::UniExpr(uni.op,
                                                   Typed::untyped(expr)),
                                                   None)
        }

        AstExpr::Literal(literal) => {
            let lit_type = match literal {
                Literal::String(_) => Some(universe.string()),
                Literal::Number(ref num) => {
                    None
                },
                Literal::Bool(_) => Some(universe.boolean()),
            };

            scope.map_tmp(universe, Value::Literal(literal), lit_type)
        }

        AstExpr::Ident(ident) => scope.map_tmp(universe, Value::Ident(Ident::new(ident)), None),

        AstExpr::FnCall(fn_call) => {
            let name = fn_call.name;
            let args = fn_call.args.map(|vec| vec.into_iter().map(|e| Typed::untyped(flatten_expr(universe, scope, e))).collect::<Vec<_>>());

            let fn_call = FnCall::new(name, args);

            scope.map_tmp(universe, Value::FnCall(fn_call), None)
        }
    }
    
}

#[derive(Debug, Clone, PartialEq)]
pub struct Typed<T> where T: ::std::fmt::Debug + Clone + PartialEq {
    data: T,
    data_type: Cell<Option<TypeId>>
}

impl<T> Typed<T> where T: ::std::fmt::Debug + Clone + PartialEq {
    fn untyped(data: T) -> Typed<T> {
        Typed {
            data: data,
            data_type: Cell::new(None) 
        }
    }

    fn typed(data: T, t: TypeId) -> Typed<T> {
        Typed {
            data: data,
            data_type: Cell::new(Some(t)),
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

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    map: HashMap<TmpId, Tmp>,
    execution_order: Vec<TmpId>,
}

impl Expr {

    pub fn get_tmp(&self, id: TmpId) -> &Tmp {
        self.map.get(&id).expect("Given ID should always be valid if taken from the correct Expr")
    }

    pub fn execution_order(&self) -> Iter<TmpId> {
        self.execution_order.iter()
    }

    fn map_tmp(&mut self, universe: &Universe, val: Value, t: Option<TypeId>) -> TmpId {
        let tmp = Tmp {
            id: universe.new_tmp_id(),
            value: Typed {
                data: val,
                data_type: Cell::new(t),
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

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Literal(Literal),
    Ident(Ident),
    FnCall(FnCall),
    BinExpr(BinOp, Typed<TmpId>, Typed<TmpId>),
    UniExpr(UniOp, Typed<TmpId>),
}

#[derive(Debug, Clone, PartialEq)]
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
    name: AstIdent,
    args: Option<Vec<Typed<TmpId>>>,
    fn_id: Cell<Option<FnId>>,
}

impl FnCall {
    fn new(name: AstIdent, args: Option<Vec<Typed<TmpId>>>) -> FnCall {
        FnCall {
            name: name,
            args: args,
            fn_id: Cell::new(None),
        }
    }

    pub fn set_id(&self, id: FnId) {
        if self.fn_id.get().is_some() {
            panic!("Attempting to overwrite {} of the FnCall {:?}", self.fn_id.get().unwrap(), self.name);
        } else {
            self.fn_id.set(Some(id));
        }
    }

    pub fn get_id(&self) -> Option<FnId> {
        self.fn_id.get()
    }
}

#[cfg(test)]
mod tests {
    use parser::*;
    use semantic_ck::*;
    use super::*;

    #[test]
    fn expr_exec_order_ck() {
        let input = "5 + 2 / 3";
        let expr = parse_Expr(input).unwrap();

        let universe = Universe::std();

        let expr = flatten(&universe, expr);

        let mut order = expr.execution_order();

        // Find and validate tmp storing 5.
        let _5_id = order.next().unwrap();
        {
            match expr.get_tmp(*_5_id).value.data {
                Value::Literal(ref literal) => {
                    assert_eq!(*literal, Literal::Number("5".to_string()));
                }

                ref v @ _ => panic!("Unexpected value {:?}. Expected a literal number 5", v),
            }
        }

        // Find and validate tmp storing 2.
        let _2_id = order.next().unwrap();
        {
            match expr.get_tmp(*_2_id).value.data {
                Value::Literal(ref literal) => {
                    assert_eq!(*literal, Literal::Number("2".to_string()));
                }

                ref v @ _ => panic!("Unexpected value {:?}. Expected a literal number 3", v),
            }
        }

        // Find and validate tmp storing 3.
        let _3_id = order.next().unwrap();
        {
            match expr.get_tmp(*_3_id).value.data {
                Value::Literal(ref literal) => {
                    assert_eq!(*literal, Literal::Number("3".to_string()));
                }

                ref v @ _ => panic!("Unexpected value {:?}. Expected a literal number 3", v),
            }
        }

        let div_id = order.next().unwrap();
        {
            let (l_id, r_id) = match expr.get_tmp(*div_id).value.data {
                Value::BinExpr(ref op, ref lhs, ref rhs) => {
                    assert_eq!(*op, BinOp::Div);
                    (lhs.data, rhs.data)
                }

                ref v @ _ => panic!("Unexpected value {:?}. Expected a division expr", v),
            };

            assert_eq!(l_id, *_2_id);
            assert_eq!(r_id, *_3_id);
        }
        
        let add_id = order.next().unwrap();
        {
            let (l_id, r_id) = match expr.get_tmp(*add_id).value.data {
                Value::BinExpr(ref op, ref lhs, ref rhs) => {
                    assert_eq!(*op, BinOp::Add);
                    (lhs.data, rhs.data)
                }

                ref v @ _ => panic!("Unexpected value {:?}. Expected an addition expr", v),
            };

            assert_eq!(l_id, *_5_id);
            assert_eq!(r_id, *div_id);

        }
    }
}
