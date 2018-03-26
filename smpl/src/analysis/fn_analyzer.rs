use std::rc::Rc;
use std::collections::HashSet;
use petgraph::graph::NodeIndex;

use ast;
use err::*;

use super::metadata::{Metadata, FnLayout};


use super::smpl_type::*;
use super::linear_cfg_traversal::*;
use super::control_flow::CFG;
use super::typed_ast::*;
use super::semantic_data::{VarId, FnId, ScopedData, TypeId, Universe};


struct FnAnalyzer<'a> {
    universe: &'a Universe,
    fn_return_type: Rc<SmplType>,
    fn_return_type_id: TypeId,
    current_scope: ScopedData,
    scope_stack: Vec<ScopedData>,

    // metadata
    locals: Vec<(VarId, TypeId)>,
}

pub fn analyze_fn(
    universe: &Universe,
    metadata: &mut Metadata,
    global_scope: &ScopedData,
    cfg: &CFG,
    fn_id: FnId,
) -> Result<(), Err> {
    let fn_return_type;
    let fn_return_type_id;
    let func = universe.get_fn(fn_id);
    let unknown_type = universe.get_type(func.type_id());
    let func_type;
    match *unknown_type {
        SmplType::Function(ref fn_type) => {
            fn_return_type = universe.get_type(fn_type.return_type.clone());
            fn_return_type_id = fn_type.return_type;
            func_type = fn_type;
        }

        ref t @ _ => panic!("{} not mapped to a function but a {:?}", fn_id, t),
    }

    let mut analyzer = FnAnalyzer {
        universe: universe,
        fn_return_type: fn_return_type,
        fn_return_type_id: fn_return_type_id,
        current_scope: global_scope.clone(),
        scope_stack: Vec::new(),
        locals: Vec::new(),
    };

    let mut param_types = Vec::new();

    // Add parameters to the current scope.
    for param in func_type.params.iter() {
        let v_id = param.var_id;
        let t_id = param.param_type;
        analyzer
            .current_scope
            .insert_var(param.name.clone(), v_id, t_id);

        param_types.push((v_id, t_id));
    }

    // Restrain lifetime of traverser to move analyzer.locals
    {
        let traverser = Traverser::new(cfg, &mut analyzer);

        traverser.traverse()?;
    }

    metadata.insert_fn_layout(fn_id, FnLayout::new(
            analyzer.locals, 
            param_types,
            fn_return_type_id));

    return_trace(cfg)
}

// TODO: maybe add reverse traverser?
fn return_trace(cfg: &CFG) -> Result<(), Err> {

    let end = cfg.end();
    let scope_exit = cfg.previous(end);

    let unknown = cfg.previous(scope_exit);

    let mut traced = HashSet::new();
    let mut node_stack = Vec::new();
    node_stack.push(unknown);

    for _ in 0..cfg.graph().node_count() {
        let to_trace = node_stack.pop();
        match to_trace {
            Some(id) => {
                if (traced.contains(&id)) == false {
                    traced.insert(id);

                    let more_to_trace = return_check_id(cfg, id)?;
                    if let Some(vec) = more_to_trace {
                        node_stack.extend(vec);
                    }
                }
            }

            None => return Ok(()),
        }
    }

    unreachable!();
}

fn return_check_id(cfg: &CFG, id: NodeIndex) -> Result<Option<Vec<NodeIndex>>, Err> {
    use super::control_flow::Node;
 
    match *cfg.node_weight(id) {
        Node::Return(_) => Ok(None),

        Node::BranchMerge(_) => Ok(Some(cfg.before_branch_merge(id))),

        Node::ExitScope => Ok(Some(vec![cfg.previous(id)])),

        _ => return Err(ControlFlowErr::MissingReturn.into()),
    }
}

fn resolve_expr(universe: &Universe, scope: &ScopedData, expr: &Expr) -> Result<TypeId, Err> {
    let mut expr_type = None;

    for tmp_id in expr.execution_order() {
        let tmp = expr.get_tmp(*tmp_id);
        let tmp_type;
        match *tmp.value().data() {
            Value::Literal(ref literal) => {
                use ast::Literal;
                match *literal {
                    Literal::Int(_) => tmp_type = universe.int(),
                    Literal::Float(_) => tmp_type = universe.float(),
                    Literal::String(_) => tmp_type = universe.string(),
                    Literal::Bool(_) => tmp_type = universe.boolean(),
                }
            }

            Value::StructInit(ref init) => {
                // Get type info
                let type_name = init.type_name();
                let unknown_type_id = scope.type_id(type_name)?;
                let unknown_type = universe.get_type(unknown_type_id);

                // Check if type is a struct.
                let struct_type_id = unknown_type_id;
                let struct_type;
                match *unknown_type {
                    SmplType::Struct(ref t) => struct_type = t,
                    _ => {
                        return Err(TypeErr::NotAStruct {
                            type_name: type_name.clone(),
                            found: struct_type_id,
                        }.into());
                    }
                }

                init.set_struct_type(struct_type_id);
                if let Err(unknown_fields) = init.set_field_init(universe) {
                     // TODO: Allow for multiple errors
                    /*let ident = struct_type.get_ident(id);
                            return Err(TypeErr::UnknownField {
                                name: ident.clone(),
                                struct_type: struct_type_id,
                            }.into());
                            */
                    // No field initializations but the struct type has fields
                    return Err(TypeErr::StructNotFullyInitialized {
                        type_name: type_name.clone(),
                        struct_type: struct_type_id,
                        missing_fields: unknown_fields
                    }.into());
                }

                
                match init.field_init() {
                    Some(init_list) => {

                        if init_list.len() != struct_type.fields.len() {
                            // Missing fields -> struct is not fully initialized
                            return Err(TypeErr::StructNotFullyInitialized {
                                type_name: type_name.clone(),
                                struct_type: struct_type_id,
                                missing_fields: {
                                    let inits = init_list
                                        .iter()
                                        .map(|&(ref name, _)| name.clone())
                                        .collect::<Vec<_>>();

                                    struct_type
                                        .field_map
                                        .iter()
                                        .filter(|&(_, ref id)| {
                                            
                                            !inits.contains(id)
                                        })
                                        .map(|(ident, _)| ident.clone())
                                        .collect::<Vec<_>>()
                                },
                            }.into());
                        }
                        // Go threw initialization list and check expressions
                        for (ref id, ref typed_tmp_id) in init_list {
                            let field_type_id = struct_type.field_type(*id).unwrap();
                            let tmp = expr.get_tmp(*typed_tmp_id.data());
                            let tmp_type_id = tmp.value().type_id().unwrap();
                            typed_tmp_id.set_type_id(tmp_type_id);

                            // Expression type the same as the field type?
                            if universe.get_type(tmp_type_id)
                                != universe.get_type(field_type_id)
                            {
                                return Err(TypeErr::UnexpectedType {
                                    found: tmp_type_id,
                                    expected: field_type_id,
                                }.into());
                            }
                        }
                    }

                    None => {
                        if struct_type.fields.len() != 0 {
                            // Missing fields -> struct is not fully initialized
                            return Err(TypeErr::StructNotFullyInitialized {
                                type_name: type_name.clone(),
                                struct_type: struct_type_id,
                                missing_fields: {
                                    struct_type
                                        .field_map
                                        .iter()
                                        .map(|(ident, _)| ident.clone())
                                        .collect::<Vec<_>>()
                                },
                            }.into());
                        }
                    }
                } 

                tmp_type = struct_type_id;
            }

            Value::Variable(ref var) => {
                let (var_id, type_id) = scope.var_info(var.ident())?;
                var.set_id(var_id);

                tmp_type = type_id;
            }

            Value::FieldAccess(ref field_access) => {
                let accessed_field_type_id =
                    resolve_field_access(universe, scope, field_access)?;

                tmp_type = accessed_field_type_id;
            }

            Value::BinExpr(ref op, ref lhs, ref rhs) => {
                let lhs_type_id = expr.get_tmp(*lhs.data()).value().type_id().unwrap();
                let rhs_type_id = expr.get_tmp(*rhs.data()).value().type_id().unwrap();

                lhs.set_type_id(lhs_type_id);
                rhs.set_type_id(rhs_type_id);

                tmp_type = resolve_bin_op(universe, op, lhs_type_id, rhs_type_id)?;
            }

            Value::UniExpr(ref op, ref uni_e) => {
                let tmp_type_id = expr.get_tmp(*uni_e.data()).value().type_id().unwrap();

                tmp_type = resolve_uni_op(universe, op, tmp_type_id)?;
            }

            Value::FnCall(ref fn_call) => {
                let fn_id = scope.get_fn(&fn_call.path().clone())?;
                let func = universe.get_fn(fn_id);
                let fn_type_id = func.type_id();
                let fn_type = universe.get_type(fn_type_id);

                fn_call.set_id(fn_id);

                if let SmplType::Function(ref fn_type) = *fn_type {
                    let arg_type_ids = fn_call.args().map(|ref vec| {
                        vec.iter()
                            .map(|ref tmp_id| {
                                let tmp = expr.get_tmp(*tmp_id.data());
                                let tmp_value = tmp.value();
                                let tmp_value_type_id = tmp_value.type_id().unwrap();
                                tmp_id.set_type_id(tmp_value_type_id);
                                tmp_value_type_id
                            })
                            .collect::<Vec<_>>()
                    });

                    match arg_type_ids {
                        Some(arg_type_ids) => {
                            if fn_type.params.len() != arg_type_ids.len() {
                                return Err(TypeErr::Arity {
                                    fn_type: fn_type_id,
                                    found_args: arg_type_ids.len(),
                                    expected_param: fn_type.params.len(),
                                }.into());
                            }

                            let fn_param_type_ids = fn_type.params.iter();

                            for (index, (arg, param)) in
                                arg_type_ids.iter().zip(fn_param_type_ids).enumerate()
                            {
                                let arg_type = universe.get_type(*arg);
                                let param_type = universe.get_type(param.param_type);
                                if arg_type != param_type {
                                    return Err(TypeErr::ArgMismatch {
                                        fn_id: fn_id,
                                        index: index,
                                        arg: *arg,
                                        param: param.param_type,
                                    }.into());
                                }
                            }
                        }

                        None => {
                            if fn_type.params.len() != 0 {
                                return Err(TypeErr::Arity {
                                    fn_type: fn_type_id,
                                    found_args: 0,
                                    expected_param: fn_type.params.len(),
                                }.into());
                            }
                        }
                    }

                    tmp_type = fn_type.return_type;
                } else {
                    panic!(
                        "{} was mapped to {}, which is not SmplType::Function but {:?}",
                        fn_id, fn_type_id, fn_type
                    );
                }
            }
        }

        tmp.value().set_type_id(tmp_type);
        expr_type = Some(tmp_type);
    }

    Ok(expr_type.unwrap())
}

fn resolve_bin_op(
    universe: &Universe,
    op: &ast::BinOp,
    lhs: TypeId,
    rhs: TypeId,
) -> Result<TypeId, Err> {
    use ast::BinOp::*;

    let lh_type = universe.get_type(lhs);
    let rh_type = universe.get_type(rhs);

    match *op {
        Add | Sub | Mul | Div | Mod | GreaterEq | LesserEq | Greater | Lesser => {
            match (&*lh_type, &*rh_type) {
                (&SmplType::Int, &SmplType::Int) => Ok(universe.int()),
                (&SmplType::Float, &SmplType::Float) => Ok(universe.float()),

                _ => Err(TypeErr::BinOp {
                    op: op.clone(),
                    expected: vec![universe.int(), universe.float()],
                    lhs: lhs,
                    rhs: rhs,
                }.into()),
            }
        }

        LogicalAnd | LogicalOr => match (&*lh_type, &*rh_type) {
            (&SmplType::Bool, &SmplType::Bool) => Ok(universe.boolean()),
            _ => Err(TypeErr::BinOp {
                op: op.clone(),
                expected: vec![universe.boolean()],
                lhs: lhs,
                rhs: rhs,
            }.into()),
        },

        Eq | InEq => {
            if *lh_type == *rh_type {
                Ok(universe.boolean())
            } else {
                Err(TypeErr::LhsRhsInEq(lhs, rhs).into())
            }
        }
    }
}

fn resolve_uni_op(
    universe: &Universe,
    op: &ast::UniOp,
    tmp_type_id: TypeId,
) -> Result<TypeId, Err> {
    use ast::UniOp::*;

    let tmp_type = universe.get_type(tmp_type_id);

    match *op {
        Negate => match &*tmp_type {
            &SmplType::Int | &SmplType::Float => Ok(tmp_type_id),
            _ => Err(TypeErr::UniOp {
                op: op.clone(),
                expected: vec![universe.int(), universe.float()],
                expr: tmp_type_id,
            }.into()),
        },

        LogicalInvert => match &*tmp_type {
            &SmplType::Bool => Ok(tmp_type_id),
            _ => Err(TypeErr::UniOp {
                op: op.clone(),
                expected: vec![universe.boolean()],
                expr: tmp_type_id,
            }.into()),
        },

        _ => unimplemented!(),
    }
}

impl<'a> Passenger<Err> for FnAnalyzer<'a> {
    fn start(&mut self, _id: NodeIndex) -> Result<(), Err> {
        Ok(())
    }

    fn end(&mut self, _id: NodeIndex) -> Result<(), Err> {
        Ok(())
    }

    fn branch_split(&mut self, _id: NodeIndex) -> Result<(), Err> {
        Ok(())
    }

    fn branch_merge(&mut self, _id: NodeIndex) -> Result<(), Err> {
        Ok(())
    }

    fn loop_head(&mut self, _id: NodeIndex) -> Result<(), Err> {
        Ok(())
    }

    fn loop_foot(&mut self, _id: NodeIndex) -> Result<(), Err> {
        Ok(())
    }

    fn cont(&mut self, _id: NodeIndex) -> Result<(), Err> {
        Ok(())
    }

    fn br(&mut self, _id: NodeIndex) -> Result<(), Err> {
        Ok(())
    }

    fn enter_scope(&mut self, _id: NodeIndex) -> Result<(), Err> {
        self.scope_stack.push(self.current_scope.clone());
        Ok(())
    }

    fn exit_scope(&mut self, _id: NodeIndex) -> Result<(), Err> {
        let popped = self.scope_stack.pop()
                                     .expect("If CFG was generated properly and the graph is being walked correctly, there should be a scope to pop");
        self.current_scope = popped;
        Ok(())
    }

    fn local_var_decl(&mut self, _id: NodeIndex, var_decl: &LocalVarDecl) -> Result<(), Err> {
        let name = var_decl.var_name().clone();
        let var_id = var_decl.var_id();
        let var_type_annotation = var_decl.type_annotation();
        let var_type_id = self.current_scope.type_id(var_type_annotation)?;
        let var_type = self.universe.get_type(var_type_id);

        let expr_type_id = resolve_expr(self.universe, &self.current_scope, var_decl.init_expr())?;
        let expr_type = self.universe.get_type(expr_type_id);

        var_decl.set_type_id(var_type_id);

        if var_type == expr_type {
            self.current_scope.insert_var(name, var_id, var_type_id);
        } else {
            return Err(TypeErr::LhsRhsInEq(var_type_id, expr_type_id).into());
        }
        
        // Local variable types metadata
        self.locals.push((var_id, var_type_id));

        Ok(())
    }

    fn assignment(&mut self, _id: NodeIndex, assignment: &Assignment) -> Result<(), Err> {
        let assignee = assignment.assignee();

        let assignee_type_id =
            resolve_field_access(self.universe, &self.current_scope, assignee)?;

        let expr_type_id = resolve_expr(self.universe, &self.current_scope, assignment.value())?;

        let assignee_type = self.universe.get_type(assignee_type_id);
        let expr_type = self.universe.get_type(expr_type_id);

        if assignee_type != expr_type {
            return Err(TypeErr::LhsRhsInEq(assignee_type_id, expr_type_id).into());
        }

        Ok(())
    }

    fn expr(&mut self, _id: NodeIndex, expr: &Expr) -> Result<(), Err> {
        resolve_expr(self.universe, &self.current_scope, expr).map(|_| ())
    }

    fn ret(&mut self, _id: NodeIndex, expr: Option<&Expr>) -> Result<(), Err> {
        let expr_type_id = match expr {
            Some(ref expr) => resolve_expr(self.universe, &self.current_scope, expr)?,

            None => self.universe.unit(),
        };

        if self.universe.get_type(expr_type_id) != self.fn_return_type {
            return Err(TypeErr::InEqFnReturn {
                expr: expr_type_id,
                fn_return: self.fn_return_type_id,
            }.into());
        }

        Ok(())
    }

    fn loop_condition(&mut self, _id: NodeIndex, condition: &Expr) -> Result<(), Err> {
        let expr_type_id = resolve_expr(self.universe, &self.current_scope, condition)?;

        if *self.universe.get_type(expr_type_id) != SmplType::Bool {
            return Err(TypeErr::UnexpectedType {
                found: expr_type_id,
                expected: self.universe.boolean(),
            }.into());
        }

        Ok(())
    }

    fn loop_start_true_path(&mut self, _id: NodeIndex) -> Result<(), Err> {
        // Do nothing
        Ok(())
    }

    fn loop_end_true_path(&mut self, _id: NodeIndex) -> Result<(), Err> {
        // Do nothing
        Ok(())
    }

    fn branch_condition(&mut self, _id: NodeIndex, condition: &Expr) -> Result<(), Err> {
        let expr_type_id = resolve_expr(self.universe, &self.current_scope, condition)?;

        if *self.universe.get_type(expr_type_id) != SmplType::Bool {
            return Err(TypeErr::UnexpectedType {
                found: expr_type_id,
                expected: self.universe.boolean(),
            }.into());
        }

        Ok(())
    }

    fn branch_start_true_path(&mut self, _id: NodeIndex) -> Result<(), Err> {
        // Do nothing
        Ok(())
    }

    fn branch_start_false_path(&mut self, _id: NodeIndex) -> Result<(), Err> {
        // Do nothing
        Ok(())
    }

    fn branch_end_true_path(&mut self, _id: NodeIndex) -> Result<(), Err> {
        // Do nothing
        Ok(())
    }

    fn branch_end_false_path(&mut self, _id: NodeIndex) -> Result<(), Err> {
        // Do nothing
        Ok(())
    }
}

fn resolve_field_access(
    universe: &Universe,
    scope: &ScopedData,
    field_access: &FieldAccess,
) -> Result<TypeId, Err> {

    let mut path_iter = field_access.path().iter();

    let root_var_name = path_iter.next().unwrap();
    let (root_var_id, root_var_type_id) = scope.var_info(root_var_name)?;

    let mut current_type_id = root_var_type_id;
    let mut current_type = universe.get_type(root_var_type_id);

    for (index, field) in path_iter.enumerate() {
        match *current_type {
            SmplType::Struct(ref struct_type) => {
                let field_id = struct_type.field_id(field).ok_or(TypeErr::UnknownField {
                    name: field.clone(),
                    struct_type: current_type_id,
                })?;
                current_type_id = struct_type.field_type(field_id).unwrap();
            }

            _ => {
                return Err(TypeErr::FieldAccessOnNonStruct {
                    path: field_access.path().clone(),
                    index: index,
                    invalid_type: current_type_id,
                    root_type: root_var_type_id,
                }.into());
            }
        }

        current_type = universe.get_type(current_type_id);
    }

    let accessed_field_type_id = current_type_id;
    field_access.set_field_type_id(accessed_field_type_id);
    field_access.set_root_var(root_var_id, root_var_type_id);

    Ok(accessed_field_type_id)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::str::FromStr;

    use ascii::*;
    use ast::*;

    use super::*;
    use super::super::semantic_data::*;
    use super::super::typed_ast::FieldAccess;

    #[test]
    fn test_walk_field_access() {
        let mut universe = Universe::std();

        let field1_id = universe.new_field_id();
        let field2_id = universe.new_field_id();

        let field_map_1 = vec![
            (ident!("field1"), field1_id),
            (ident!("field2"), field2_id),
        ].into_iter()
            .collect::<HashMap<_, _>>();

        let fields_1 = vec![
            (field1_id, universe.int()),
            (field2_id, universe.boolean()),
        ].into_iter()
            .collect::<HashMap<_, _>>();
        let struct_type_1 = StructType {
            name: ident!("foo"),
            fields: fields_1,
            field_map: field_map_1
        };
        let struct_type_1_id = universe.new_type_id();
        universe.insert_type(struct_type_1_id, SmplType::Struct(struct_type_1));


        let foo_field_id = universe.new_field_id();
        let fields_2 = vec![(foo_field_id, struct_type_1_id)]
            .into_iter()
            .collect::<HashMap<_, _>>();
        let field_map_2 = vec![(ident!("foo_field"), foo_field_id)]
            .into_iter()
            .collect::<HashMap<_, _>>();

        let struct_type_2 = StructType {
            name: ident!("bar"),
            fields: fields_2,
            field_map: field_map_2,
        };
        let struct_type_2_id = universe.new_type_id();
        universe.insert_type(struct_type_2_id, SmplType::Struct(struct_type_2));

        let mut scope = universe.std_scope();
        let root_var_id = universe.new_var_id();
        scope.insert_var(ident!("baz"), root_var_id, struct_type_2_id);

        let path = path!("baz", "foo_field", "field1");
        let access = FieldAccess::new(path);
        let field_type_id = resolve_field_access(&universe, &scope, &access).unwrap();

        assert_eq!(field_type_id, universe.int());
    }
}
