use std::collections::HashMap;

use petgraph::graph::NodeIndex;

use crate::ast;
use crate::span::Span;

use super::linear_cfg_traversal::*;
use super::control_data::*;
use super::control_flow::CFG;
use super::semantic_data::*;
use super::semantic_data::Function;
use super::error::*;
use super::typed_ast::*;
use super::type_cons::*;
use super::type_resolver;
use super::resolve_scope::ScopedData;
use super::analysis_helpers;

pub fn type_check(universe: &mut Universe, fn_id: FnId) -> Result<(), AnalysisError> {
    use super::semantic_data::Function;

    let cfg = {
        let fn_to_resolve = universe.get_fn(fn_id);
        match fn_to_resolve {
            Function::SMPL(ref smpl_fn) => smpl_fn.cfg(),
            Function::Anonymous(ref afn) => {
                match afn {
                    AnonymousFunction::Reserved(_) => panic!("Anonymous function should be resolved"),

                    AnonymousFunction::Resolved {
                        ref cfg,
                        ..
                    } => {
                        cfg.clone()
                    }
                }
            }

            _ => panic!("Not a function with a type-checkable body"),
        }
    };

    let mut type_checker = TypeChecker::new(universe, fn_id)?;
    let cfg = cfg.borrow();
    let mut traverser = Traverser::new(&*cfg, &mut type_checker);
    traverser.traverse()
}

struct TypeChecker<'a> {
    universe: &'a mut Universe,
    scopes: Vec<ScopedData>,
    typing_context: TypingContext,
    return_type: AbstractType,
}

impl<'a> TypeChecker<'a> {

    // TODO: Store function (return) type somwhere
    // TODO: Add function parameters somewhere
    // TODO: Put formal parameters into function scope within Universe
    pub fn new(universe: &mut Universe, fn_id: FnId) -> Result<TypeChecker, AnalysisError> {

        use super::semantic_data::Function;

        match universe.get_fn(fn_id) {

            Function::Builtin(_) => unimplemented!(),

            Function::Anonymous(anonymous_fn) => {
                match anonymous_fn {
                    AnonymousFunction::Reserved(..) => {
                        panic!("Expected anonymous functions to already be resolved");
                    }

                    AnonymousFunction::Resolved {
                        ref fn_type,
                        ref analysis_context,
                        ..
                    } => {
                        let typing_context = analysis_context.typing_context().clone();
                        let fn_scope = analysis_context.fn_scope().clone();

                        let return_type: AbstractType = {
                            
                            let type_id = fn_type.clone();

                            let fn_type = AbstractType::App {
                                type_cons: type_id,
                                args: analysis_context
                                    .existential_type_vars()
                                    .iter()
                                    .map(|id| AbstractType::TypeVar(id.clone()))
                                    .collect::<Vec<_>>(),
                            }.substitute(universe)?;

                            match fn_type {
                                AbstractType::Function {
                                    ref return_type,
                                    ..
                                } => *return_type.clone(),

                                _ => panic!("Non-function type constructor for function"),
                            }
                        };
                        
                        Ok(TypeChecker {
                            scopes: vec![fn_scope],
                            typing_context: typing_context,
                            universe: universe,
                            return_type: return_type,
                        })
                    }
                }
            }

            Function::SMPL(smpl_function) => {

                let typing_context = smpl_function.analysis_context().typing_context().clone();
                let fn_scope = smpl_function.analysis_context().fn_scope().clone();

                let return_type: AbstractType = {
                        
                    let type_id = smpl_function.fn_type();

                    let fn_type = AbstractType::App {
                        type_cons: type_id,
                        args: smpl_function.analysis_context()
                            .existential_type_vars()
                            .iter()
                            .map(|id| AbstractType::TypeVar(id.clone()))
                            .collect::<Vec<_>>(),
                    }.substitute(universe)?;

                    match fn_type {
                        AbstractType::Function {
                            ref return_type,
                            ..
                        } => *return_type.clone(),

                        _ => panic!("Non-function type constructor for function"),
                    }
                };

                dbg!(&return_type);

                Ok(TypeChecker {
                    scopes: vec![fn_scope],
                    typing_context: typing_context,
                    universe: universe,
                    return_type: return_type,
                })
            }
        }
    }

    fn current(&self) -> &ScopedData {
        self.scopes
            .last()
            .expect("Should always have a scope")
    }

    fn current_mut(&mut self) -> &mut ScopedData {
        self.scopes
            .last_mut()
            .expect("Should always have a scope")
    }

    fn fork_current(&mut self) {
        let fork = self.current().clone();
        self.scopes.push(fork);
    }

    fn pop_current(&mut self) -> ScopedData {
        self.scopes
            .pop()
            .expect("Should always have a scope")
    }
}

macro_rules! expr_type {
    ($self: expr, $expr: expr) => {{
        resolve_expr($self.universe, 
            $self.scopes
                .last()
                .expect("Should always have a scope"),
            &mut $self.typing_context,
            $expr)
    }}
}

macro_rules! resolve {
    ($self: expr, $synthesis: expr, $constraint: expr, $span: expr) => {{
        use super::type_resolver;
        type_resolver::resolve_types(
            $self.universe,
            $self.scopes
                .last()
                .expect("Should always have a scope"),
            &mut $self.typing_context,
            $synthesis,
            $constraint,
            $span)
    }}
}

macro_rules! ann_to_type {
    ($self: expr, $ann: expr) => {{
        use super::type_cons;
        type_cons::type_from_ann(
            $self.universe,
            $self.scopes
                .last()
                .expect("Should always have a scope"),
            &$self.typing_context,
            $ann)
    }}
}

type E = AnalysisError;
impl<'a> Passenger<E> for TypeChecker<'a> {
    fn start(&mut self, id: NodeIndex) -> Result<(), E> {
        Ok(())
    }

    fn end(&mut self, id: NodeIndex) -> Result<(), E> {
        Ok(())
    }

    fn loop_head(&mut self, id: NodeIndex, ld: &LoopData, expr: &ExprData) 
        -> Result<(), E> {
       
        let expr_type = expr_type!(self, &expr.expr)?;
        resolve!(self, &expr_type, &AbstractType::Bool, expr.span)?;

        Ok(())
    }

    fn loop_foot(&mut self, id: NodeIndex, ld: &LoopData) -> Result<(), E> {
        Ok(())
    }

    fn cont(&mut self, id: NodeIndex, ld: &LoopData) -> Result<(), E> {
        Ok(())
    }

    fn br(&mut self, id: NodeIndex, ld: &LoopData) -> Result<(), E> {
        Ok(())
    }

    fn enter_scope(&mut self, id: NodeIndex) -> Result<(), E> {
        self.fork_current();
        Ok(())
    }

    fn exit_scope(&mut self, id: NodeIndex) -> Result<(), E> {
        let _old_scope = self.pop_current();
        Ok(())
    }

    fn local_var_decl(&mut self, id: NodeIndex, decl: &LocalVarDeclData) -> Result<(), E> {
        let var_decl = &decl.decl;

        let expr_type = expr_type!(self, var_decl.init_expr())?;

        let var_type = match var_decl.type_annotation() {
            Some(ann) => {
                let ann_type = ann_to_type!(self, ann)?;
                resolve!(self, &expr_type, &ann_type, decl.span)?;

                ann_type
            }

            None => {
                // No type annotation
                // Default to the RHS type
                expr_type
            }
        };

        self.typing_context.var_type_map
            .insert(var_decl.var_id(), var_type);

        Ok(())
    }

    fn assignment(&mut self, id: NodeIndex, assign: &AssignmentData) -> Result<(), E> {
        let assignment = &assign.assignment;

        let value_type = expr_type!(self, assignment.value())?;

        let assignee_type = resolve_field_access(
            self.universe,
            self.scopes
                .last()
                .expect("Should always have a scope"),
            &mut self.typing_context,
            assignment.assignee(),
            assignment.access_span()
        )?;

        resolve!(self, &value_type, &assignee_type, assign.span)?;

        Ok(())
    }

    fn expr(&mut self, id: NodeIndex, expr: &ExprData) -> Result<(), E> {
        let _expr_type = expr_type!(self, &expr.expr)?;

        Ok(())
    }

    fn ret(&mut self, id: NodeIndex, rdata: &ReturnData) -> Result<(), E> {
        // TODO: Resolve types of expression
        // TODO: Check if return type compatible

        match rdata.expr {
            Some(ref expr) => {
                let expr_type = expr_type!(self, expr)?;
                dbg!(&expr_type, &self.return_type);
                resolve!(self, &expr_type, &self.return_type, rdata.span)
                    .map_err(|e| e.into())
            }

            None => {
                resolve!(self, &AbstractType::Unit, &self.return_type, rdata.span)
                    .map_err(|e| e.into())
            }
        }
    }

    fn loop_start_true_path(&mut self, id: NodeIndex) -> Result<(), E> {
        Ok(())
    }

    fn loop_end_true_path(&mut self, id: NodeIndex) -> Result<(), E> {
        Ok(())
    }

    fn branch_split(&mut self, id: NodeIndex, b: &BranchingData, e: &ExprData) 
        -> Result<(), E> {

        let expr_type = expr_type!(self, &e.expr)?;
        resolve!(self, &expr_type, &AbstractType::Bool, e.span)?;

        Ok(())
    }

    fn branch_merge(&mut self, id: NodeIndex, b: &BranchingData) -> Result<(), E> {
        Ok(())
    }

    fn branch_start_true_path(&mut self, id: NodeIndex) -> Result<(), E> {
        Ok(())
    }

    fn branch_start_false_path(&mut self, id: NodeIndex) -> Result<(), E> {
        Ok(())
    }

    fn branch_end_true_path(&mut self, id: NodeIndex, b: &BranchingData) -> Result<(), E> {
        Ok(())
    }

    fn branch_end_false_path(&mut self, id: NodeIndex, b: &BranchingData) -> Result<(), E> {
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct TypingContext {
    pub type_vars: HashMap<TypeVarId, AbstractType>,
    pub var_type_map: HashMap<VarId, AbstractType>,
    pub fn_type_map: HashMap<FnId, AbstractType>,
    pub tmp_type_map: HashMap<TmpId, AbstractType>,
}

impl TypingContext {

    pub fn empty() -> TypingContext {
        TypingContext {
            type_vars: HashMap::new(),
            var_type_map: HashMap::new(),
            fn_type_map: HashMap::new(),
            tmp_type_map: HashMap::new(),
        }
    }
    pub fn get_type_var(&self, id: TypeVarId) -> Option<&AbstractType> {
        self.type_vars
            .get(&id)
    }
}

fn resolve_expr(universe: &mut Universe, scope: &ScopedData, context: &mut TypingContext, expr: &Expr) 
    -> Result<AbstractType, AnalysisError> {

    let mut expr_type = None;
    for tmp_id in expr.execution_order() {
        let tmp = expr.get_tmp(tmp_id);

        let tmp_type = resolve_tmp(universe, scope, context, expr, tmp)?;
        expr_type = Some(tmp_type.clone());

        if context.tmp_type_map
            .insert(tmp_id, tmp_type)
            .is_some() {
            panic!("Duplicate tmp ID"); 
        }
    }

    Ok(expr_type.unwrap())
}

fn resolve_tmp(universe: &mut Universe, scope: &ScopedData, 
    context: &mut TypingContext, expr: &Expr, tmp: &Tmp) 
    -> Result<AbstractType, AnalysisError> {

    let tmp_span = tmp.span();
    let tmp_value = tmp.value();
    let tmp_type = match tmp_value.data() {
        Value::Literal(ref literal) => {
            match *literal {
                Literal::Int(_) => AbstractType::Int,
                Literal::Float(_) => AbstractType::Float,
                Literal::String(_) => AbstractType::String,
                Literal::Bool(_) => AbstractType::Bool,
            }
        }

        Value::BinExpr(ref op, ref lhs, ref rhs) => {
            // TODO: These clones are necessary b/c typing context may mutate
            //  If type inference becomes a thing, the function will need to be
            //    re-typechecked. No type inference ATM so nothing to do
            let lhs_type = context
                .tmp_type_map
                .get(lhs.data())
                .expect("Missing tmp")
                .substitute(universe)?;
            let rhs_type = context
                .tmp_type_map
                .get(rhs.data())
                .expect("Missing tmp")
                .substitute(universe)?;

            resolve_bin_op(
                universe,
                scope, 
                context, 
                op, 
                &lhs_type, 
                &rhs_type,
                tmp_span)?
        }

        Value::UniExpr(ref op, ref uni_tmp) => {
            let uni_tmp_type = context
                .tmp_type_map
                .get(uni_tmp.data())
                .expect("Missing tmp")
                .substitute(universe)?;

            resolve_uni_op(scope, context, op, &uni_tmp_type, tmp.span())?
        }

        Value::StructInit(ref init) => {
            resolve_struct_init(universe, scope, context, init, tmp.span())? 
        }

        Value::AnonStructInit(ref init) => {
            resolve_anon_struct_init(universe, scope, context, init, tmp.span())?
        }

        Value::Binding(ref binding) => {
            resolve_binding(universe, scope, context, binding, tmp.span())?
        }

        Value::ModAccess(ref access) => {
            resolve_mod_access(universe, scope, context, access, tmp.span())?
        }

        Value::FnCall(ref fn_call) => {
            resolve_fn_call(universe, scope, context, fn_call, tmp.span())?
        }

        Value::ArrayInit(ref init) => {
            resolve_array_init(universe, scope, context, init, tmp.span())?
        }

        Value::Indexing(ref indexing) => {
            resolve_indexing(universe, scope, context, indexing, tmp.span())?
        }

        Value::TypeInst(ref type_inst) => {
            resolve_type_inst(universe, scope, context, type_inst, tmp.span())?
        }

        Value::AnonymousFn(ref a_fn) => {
            resolve_anonymous_fn(universe, scope, context, a_fn, tmp.span())?
        }

        Value::FieldAccess(ref field_access) => {
            resolve_field_access(universe, 
                scope, 
                context, 
                field_access, 
                tmp.span())?
        }
    }; 

    Ok(tmp_type)
}

/// Assume types are already applied
fn resolve_bin_op(
    universe: &Universe,
    scope: &ScopedData,
    context: &mut TypingContext,
    op: &ast::BinOp,
    lhs: &AbstractType,
    rhs: &AbstractType,
    span: Span,
) -> Result<AbstractType, AnalysisError> {
    use crate::ast::BinOp::*;

    let expected_int = AbstractType::Int;
    let expected_float = AbstractType::Float;
    let expected_bool = AbstractType::Bool;

    let resolve_type = match *op {
        Add | Sub | Mul | Div | Mod => match (&lhs, &rhs) {
            (&AbstractType::Int, &AbstractType::Int) => AbstractType::Int,
            (&AbstractType::Float, &AbstractType::Float) => AbstractType::Float,

            _ => {
                return Err(TypeError::BinOp {
                    op: op.clone(),
                    expected: vec![expected_int, expected_float],
                    lhs: lhs.clone(),
                    rhs: rhs.clone(),
                    span: span,
                }
                .into());
            }
        },

        LogicalAnd | LogicalOr => match (&lhs, &rhs) {
            (&AbstractType::Bool, &AbstractType::Bool) => AbstractType::Bool,
            _ => {
                return Err(TypeError::BinOp {
                    op: op.clone(),
                    expected: vec![expected_bool],
                    lhs: lhs.clone(),
                    rhs: rhs.clone(),
                    span: span,
                }
                .into());
            }
        },

        GreaterEq | LesserEq | Greater | Lesser => match (&lhs, &rhs) {
            (&AbstractType::Int, &AbstractType::Int) => AbstractType::Bool,
            (&AbstractType::Float, &AbstractType::Float) => AbstractType::Bool,

            _ => {
                return Err(TypeError::BinOp {
                    op: op.clone(),
                    expected: vec![expected_int, expected_float],
                    lhs: lhs.clone(),
                    rhs: rhs.clone(),
                    span: span,
                }
                .into());
            }
        },

        Eq | InEq => {

            // TODO: Stricter equality check?
            type_resolver::resolve_types(universe,
                scope,
                context,
                lhs,
                rhs,
                span)?;

            AbstractType::Bool
        }
    };

    Ok(resolve_type)
}

/// Assume types are already applied
fn resolve_uni_op(
    scope: &ScopedData,
    context: &TypingContext,
    op: &ast::UniOp,
    tmp_type: &AbstractType,
    span: Span,
) -> Result<AbstractType, AnalysisError> {
    use crate::ast::UniOp::*;

    let expected_int = AbstractType::Int;

    let expected_float = AbstractType::Float;

    let expected_bool = AbstractType::Bool;

    match *op {
        Negate => match tmp_type {
            AbstractType::Int | AbstractType::Float => Ok(tmp_type.clone()),
            _ => Err(TypeError::UniOp {
                op: op.clone(),
                expected: vec![expected_int, expected_float],
                expr: tmp_type.clone(),
                span: span,
            }
            .into()),
        },

        LogicalInvert => match tmp_type {
            AbstractType::Bool => Ok(tmp_type.clone()),
            _ => Err(TypeError::UniOp {
                op: op.clone(),
                expected: vec![expected_bool],
                expr: tmp_type.clone(),
                span: span,
            }
            .into()),
        },

        _ => unimplemented!(),
    }
}

fn resolve_struct_init(universe: &Universe, scope: &ScopedData, 
    context: &mut TypingContext, init: &StructInit, span: Span) 
    -> Result<AbstractType, AnalysisError> {

    // Get type info
    let type_name = init.type_name();
    let tmp_type_name = type_name.clone().into();
    let struct_type_id = scope
        .type_cons(universe, &tmp_type_name)
        .ok_or(AnalysisError::UnknownType(type_name.clone()))?;

    let type_args = init.type_args()
        .map(|vec| {
            vec
            .iter()
            .map(|ann| {
                type_from_ann(
                    universe,
                    scope,
                    context,
                    ann,
                )
            })
            .collect::<Result<Vec<_>, _>>()
        })
        .unwrap_or(Ok(Vec::new()))?;

    // TODO: Take into account type arguments
    let struct_type = AbstractType::App {
        type_cons: struct_type_id,
        args: type_args,
    }
    .substitute(universe)?;

    // Check if type is a struct.
    let (struct_type_id, fields, field_map) = match struct_type {
        AbstractType::Record {
            type_id: struct_type_id,
            ref abstract_field_map,
            ..
        } => (struct_type_id, &abstract_field_map.fields, &abstract_field_map.field_map),

        _ => {
            return Err(TypeError::NotAStruct {
                type_name: type_name.clone(),
                found: struct_type,
                span: span,
            }
            .into());
        }
    };

    // Check if the struct is an 'opaque' type (i.e. cannot be initialized by SMPL
    // code)
    // TODO: Opaque check
    /*
    if self.program.metadata().is_opaque(struct_type_id) {
        return Err(TypeError::InitOpaqueType {
            struct_type: struct_type,
            span: tmp.span(),
        }
        .into());
    }
    */

    // Map init'd field to its type
    let mut init_expr_type_map: HashMap<FieldId, AbstractType> = HashMap::new();
    for (field_name, typed_tmp) in init.raw_field_init() {

        // Check if the struct type has the corresponding field
        let field_id = field_map
            .get(field_name)
            .ok_or(TypeError::UnknownField {
                name: field_name.clone(),
                struct_type: struct_type.clone(),
                span: span,
            })?;

        let tmp_type = context.tmp_type_map
            .get(typed_tmp.data())
            .expect("Missing tmp")
            .clone();

        if init_expr_type_map.insert(field_id.clone(), tmp_type).is_some() {
            panic!("Duplicate field init");
        }
    }

    // Not a full struct init
    if init_expr_type_map.len() != fields.len() {

        let missing_fields = field_map
            .iter()
            .filter(|(_, field_id)| init_expr_type_map.contains_key(field_id))
            .map(|(ident, _)| ident.clone())
            .collect();

        return Err(TypeError::StructNotFullyInitialized {
            type_name: type_name.clone(),
            struct_type: struct_type.clone(),
            missing_fields: missing_fields,
            span: span,
        }.into());
    }

    // SATISFIED CONDITIONS: 
    //   Field init expressions should be fully typed (tmps)
    //   Field names are all present and all valid

    // Check if field init expressions are of the correct type
    for (field_id, field_type) in fields.iter() {
        let init_expr_type = init_expr_type_map
            .get(field_id)
            .unwrap();

        // TODO: If type inference is implemented, another pass needs to check
        //   that all types are still valid
        type_resolver::resolve_types(
            universe,
            scope,
            context,
            init_expr_type,
            field_type,
            span)?;
    }

    // SATISFIED CONDITIONS: 
    //   Field init expressions should be fully typed (tmps)
    //   Field names are all present and all valid
    //   Field init expressions are valid types for their corresponding fields

    Ok(struct_type)
}

/// Generates a WidthConstraint based on the types of its initializer expressions
fn resolve_anon_struct_init(universe: &Universe, scope: &ScopedData, 
    context: &TypingContext, init: &AnonStructInit, span: Span) 
    -> Result<AbstractType, AnalysisError> {

    let mut width_constraint = AbstractWidthConstraint {
        fields: HashMap::new(),
    };

    // Map init'd field to its type
    let mut duplicate_fields = Vec::new();
    for (field_name, typed_tmp) in init.raw_field_init() {

        let tmp_type = context.tmp_type_map
            .get(typed_tmp)
            .expect("Missing tmp");

        if width_constraint.fields.contains_key(field_name) {
            duplicate_fields.push(field_name.clone());
        } else {
            width_constraint.fields.insert(field_name.clone(), tmp_type.clone());
        }
    }

    if duplicate_fields.len() != 0 {
        return Err(TypeError::InvalidInitialization {
            fields: duplicate_fields,
            span: span,
        }.into());
    }

    let width_type = AbstractType::WidthConstraint(width_constraint);
    Ok(width_type)
}

fn resolve_binding(universe: &Universe, scope: &ScopedData,
    context: &TypingContext, binding: &Binding, span: Span)
    -> Result<AbstractType, AnalysisError> {

    match binding.get_id().unwrap() {
        BindingId::Var(var_id) => {
            Ok(context
                .var_type_map
                .get(&var_id)
                .expect("Missing VarId")
                .clone())
        }

        BindingId::Fn(fn_id) => {
            // self.program.features_mut().add_feature(FUNCTION_VALUE);

            // Bindings to unchecked functions are OK because:
            // 1) Attempting to use the binding will trigger type checking
            // 2) Cannot write out unchecked function types currently
            /*
            if self.program
                .metadata_mut()
                .is_builtin_params_unchecked(fn_id)
            {
                return Err(AnalysisError::UncheckedFunctionBinding(var.ident().clone()));
            }
            */

            // TODO: builtin check
            let fn_type_id = universe
                .get_fn(fn_id)
                .fn_type()
                .expect("Expect anonymous function types to already be resolved")
                .clone();
            /*
            let fn_type_id = if self.program.metadata().is_builtin(fn_id) {
                let f = self.program.universe().get_builtin_fn(fn_id);
                f.fn_type().clone()
            } else {
                let f = self.program.universe().get_fn(fn_id);
                f.fn_type().clone()
            };
            */

            let fn_type = AbstractType::App {
                type_cons: fn_type_id,
                args: Vec::new(),
            }
            .substitute(universe)?;

            Ok(fn_type)
        }
    }
}

fn resolve_mod_access(universe: &Universe, scope: &ScopedData,
    context: &TypingContext, mod_access: &ModAccess, span: Span)
    -> Result<AbstractType, AnalysisError> {

    let fn_id = mod_access.fn_id()
        .expect("Should be set by name resolution");

    // TODO: Builtin detection
    /*
    let fn_type_id = if self.program.metadata().is_builtin(fn_id) {
        let f = self.program.universe().get_builtin_fn(fn_id);
        f.fn_type().clone()
    } else {
        let f = self.program.universe().get_fn(fn_id);
        f.fn_type().clone()
    };
    */

    let fn_type_id = universe
        .get_fn(fn_id)
        .fn_type()
        .expect("Expect anonymous functions to already be resolved");

    let fn_type = AbstractType::App {
        type_cons: fn_type_id,
        args: Vec::new(),
    }
    .substitute(universe)?;

    Ok(fn_type)
}

fn resolve_fn_call(universe: &Universe, scope: &ScopedData, context: &TypingContext,
    fn_call: &FnCall, span: Span)
    -> Result<AbstractType, AnalysisError> {

    let fn_value = fn_call.fn_value();
    let fn_value_type = context.tmp_type_map
        .get(&fn_value)
        .expect("Missing TMP");

    // Check args and parameters align
    match fn_value_type {
        AbstractType::Function {
            parameters: ref params,
            ref return_type,
        } => {
            let arg_types = fn_call.args().map(|ref vec| {
                vec.iter()
                    .map(|ref tmp_id| {
                        context.tmp_type_map
                            .get(tmp_id.data())
                            .expect("Missing TMP")
                            .clone()
                    })
                    .collect::<Vec<_>>()
            });

            match arg_types {
                Some(arg_types) => {
                    if params.len() != arg_types.len() {
                        return Err(TypeError::Arity {
                            fn_type: fn_value_type.clone(),
                            found_args: arg_types.len(),
                            expected_param: params.len(),
                            span: span,
                        }
                        .into());
                    }

                    let fn_param_types = params.iter();

                    for (index, (arg_type, param_type)) in
                        arg_types.iter().zip(fn_param_types).enumerate()
                    {
                        let arg_type: &AbstractType = arg_type;
                        let param_type: &AbstractType = param_type;
                        // TODO: Check if types can resolve
                        /*
                        if !resolve_types(&arg_type, &param_type) {
                            return Err(TypeError::ArgMismatch {
                                fn_type: fn_value_type.clone(),
                                index: index,
                                arg: arg_type.clone(),
                                param: param_type.clone(),
                                span: span,
                            }
                            .into());
                        }
                        */
                    }

                    Ok(*(return_type.clone()))
                }

                None => {
                    if params.len() != 0 {
                        Err(TypeError::Arity {
                            fn_type: fn_value_type.clone(),
                            found_args: 0,
                            expected_param: params.len(),
                            span: span,
                        }
                        .into())
                    } else {
                        Ok(*(return_type.clone()))
                    }
                }
            }
        }

        AbstractType::UncheckedFunction {
            return_type,
            ..
        } => {
            Ok(*(return_type.clone()))
        }

        t @ _ => panic!("Function call on a non-function type: {:?}", t),
    }
}

fn resolve_array_init(universe: &Universe, scope: &ScopedData, context: &mut TypingContext,
    init: &ArrayInit, span: Span)
    -> Result<AbstractType, AnalysisError> {

    match *init {
        ArrayInit::List(ref vec) => {
            let size = vec.len() as u64;
            let element_types = vec.iter().map(|ref tmp_id| {
                let tmp_type = context.tmp_type_map
                    .get(tmp_id.data())
                    .expect("Missing TMP")
                    .clone();
                (tmp_type, span)
            }).collect::<Vec<_>>();

            let mut expected_element_type = None;

            for (i, (current_element_type, span)) in element_types.into_iter().enumerate() {
                if expected_element_type.is_none() {
                    expected_element_type = Some(
                        current_element_type
                            .substitute(universe)?
                    );
                    continue;
                }

                let expected_element_type = expected_element_type
                    .as_ref()
                    .unwrap();

                type_resolver::resolve_types(
                    universe,
                    scope,
                    context,
                    &current_element_type, 
                    expected_element_type,
                    span)
                    .map_err(|_| TypeError::HeterogenousArray {
                        expected: expected_element_type.clone(),
                        found: current_element_type.clone(),
                        index: i,
                        span: span,
                    })?;
            }

            let array_type = AbstractType::Array {
                element_type: Box::new(expected_element_type.unwrap().clone()),
                size: size,
            };

            Ok(array_type)
        }

        ArrayInit::Value(ref val, size) => {
            let element_type = context.tmp_type_map
                .get(val.data())
                .expect("Missing TMP");

            let array_type = AbstractType::Array {
                element_type: Box::new(element_type.clone()),
                size: size,
            };

            // TODO: Insert array type into metadata?
            /*
            self.program
                .metadata_mut()
                .insert_array_type(self.module_id, array_type);
            */
            Ok(array_type)
        }
    }
}

fn resolve_indexing(universe: &Universe, scope: &ScopedData, context: &TypingContext,
    indexing: &Indexing, span: Span) 
    -> Result<AbstractType, AnalysisError> {

    let expected_element_type: AbstractType = {
        // Check type is array
        let tmp_type = context.tmp_type_map
            .get(indexing.array.data())
            .expect("Missing TMP");

        // TODO: Already applied?
        match &tmp_type {
            AbstractType::Array {
                ref element_type,
                ..
            } => *(element_type.clone()),

            _ => {
                return Err(TypeError::NotAnArray {
                    found: tmp_type.clone(),
                    span: span,
                }
                .into());
            }
        }
    };

    {
        // Check type of indexer
        let tmp_type = context.tmp_type_map
            .get(indexing.indexer.data())
            .expect("Missing TMP");

        // TODO: Already applied?
        match &tmp_type {
            AbstractType::Int => (),

            _ => {
                return Err(TypeError::InvalidIndex {
                    found: tmp_type.clone(),
                    span: span,
                }
                .into());
            }
        }
    }

    Ok(expected_element_type)
}

fn resolve_type_inst(universe: &Universe, scope: &ScopedData, context: &TypingContext,
    type_inst: &TypeInst, span: Span) 
    -> Result<AbstractType, AnalysisError> {

    let fn_id = type_inst
        .get_id()
        .expect("No FN ID. Should be caught in scope resolution");

    let fn_type_id = universe
        .get_fn(fn_id)
        .fn_type()
        .expect("Expect anonymous functions to already be resolved");

    let type_args = type_inst
        .args()
        .iter()
        .map(|ann| {
            type_from_ann(
                universe,
                scope,
                context,
                ann,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let inst_type = AbstractType::App {
        type_cons: fn_type_id,
        args: type_args,
    }
    .substitute(universe)?;

    Ok(inst_type)
}

fn resolve_anonymous_fn(universe: &mut Universe, scope: &ScopedData, context: &TypingContext,
    a_fn: &AnonymousFn, span: Span)
    -> Result<AbstractType, AnalysisError> {

    use std::rc::Rc;
    use std::cell::RefCell;

    let fn_id = a_fn.fn_id();

    let mut resolved = None;
    if let Function::Anonymous(ref afn) = universe.get_fn(fn_id) {

        if let AnonymousFunction::Reserved(ref ast_anonymous_fn) = afn {
            let fn_type_cons =
                super::type_cons_gen::generate_anonymous_fn_type(
                    universe, scope, context, fn_id, ast_anonymous_fn)?; 

            let analysis_context = 
                analysis_helpers::generate_fn_analysis_data(
                    universe, scope, context, &fn_type_cons, ast_anonymous_fn)?;

            let cfg = super::control_flow::CFG::generate(
                universe,
                ast_anonymous_fn.body.clone(),
                &fn_type_cons,
                &analysis_context,
            )?;

            let fn_type_id = universe.insert_type_cons(fn_type_cons);

            resolved = Some(AnonymousFunction::Resolved {
                fn_type: fn_type_id,
                analysis_context: analysis_context,
                cfg: Rc::new(RefCell::new(cfg)),
            });
        }

    } else {
        panic!("FN ID did not refer to an anonymous function");
    }

    if let Some(resolved) = resolved {
        *universe.get_fn_mut(fn_id) = Function::Anonymous(resolved);
        analysis_helpers::analyze_fn(universe, fn_id)?;
    }

    let fn_type = if let Function::Anonymous(ref afn) = universe.get_fn(fn_id) {
        if let AnonymousFunction::Resolved {
            ref fn_type,
            ..
        } = afn {

            let fn_type_cons = universe.get_type_cons(fn_type.clone());
            
            // TODO: Anonymous functions are not allowed to have type parameters
            AbstractType::App {
                type_cons: fn_type.clone(),
                args: Vec::new(),
            }.substitute(universe)?

        } else {
            panic!("Anonymous function should be resolved.");
        }
    } else {
        panic!("FN ID did not refer to an anonymous function");
    };

    Ok(fn_type)
}

/// Assumes that all previous temporaries in Expr are already typed
fn resolve_field_access(
    universe: &mut Universe,
    scope: &ScopedData,
    context: &mut TypingContext,
    field_access: &FieldAccess,
    span: Span,
) -> Result<AbstractType, AnalysisError> {

    let path = field_access.path();
    let path_iter = path.path().iter();

    let root_var_id = path.root_var_id();
    let root_var_type = context.var_type_map
        .get(&root_var_id)
        .expect("Missing VAR")
        .clone();

    let mut current_type: AbstractType = root_var_type.clone();

    if let Some(expr) = path.root_indexing_expr() {
        let indexing_type = resolve_expr(universe, scope, context,expr)?;

        match indexing_type.substitute(universe)? {
            AbstractType::Int => (),
            _ => {
                return Err(TypeError::InvalidIndex {
                    found: indexing_type.clone(),
                    span: expr.get_tmp(expr.last()).span(),
                }
                .into());
            }
        }

        match current_type.substitute(universe)? {
            AbstractType::Array {
                element_type,
                ..
            } => {
                current_type = *(element_type.clone());
            }
            _ => {
                return Err(TypeError::NotAnArray {
                    found: root_var_type.clone(),
                    span: expr.get_tmp(expr.last()).span(),
                }
                .into());
            }
        }
    }

    for (index, field) in path_iter.enumerate() {
        let next_type: AbstractType;
        let field_type_retriever: 
            Box<dyn Fn(&crate::ast::Ident) -> Result<AbstractType, AnalysisError>> = 
            match current_type.substitute(universe)? {

            AbstractType::WidthConstraint(awc) => {
                Box::new(move |name| {
                    awc
                        .fields
                        .get(name)
                        .map(|t| t.clone())
                        .ok_or(TypeError::UnknownField {
                            name: name.clone(),
                            struct_type: AbstractType::WidthConstraint(awc.clone()),
                            span: span,
                        }.into())
                })
            }

            AbstractType::Record {
                type_id,
                abstract_field_map: afm,
            } => {
                let fields = afm.fields;
                let field_map = afm.field_map;

                Box::new(move |name| {
                    let field_id = field_map
                        .get(name)
                        .map(|t| t.clone())
                        .ok_or(TypeError::UnknownField {
                            name: name.clone(),
                            struct_type: AbstractType::Record {
                                type_id: type_id,
                                abstract_field_map: AbstractFieldMap {
                                    fields: fields.clone(),
                                    field_map: field_map.clone(),
                                }
                            },
                            span: span,
                        })?;

                    let field_type = fields
                        .get(&field_id)
                        .map(|t| t.clone())
                        .unwrap();

                    Ok(field_type)
                })
            }

            _ => {
                return Err(TypeError::FieldAccessOnNonStruct {
                    path: field_access.raw_path().clone(),
                    index: index,
                    invalid_type: current_type,
                    root_type: root_var_type.clone(),
                    span: span,
                }
                .into());
            }
        };

        match *field {
            PathSegment::Ident(ref field) => {
                next_type = field_type_retriever(field.name())?
                    .clone();
            }

            PathSegment::Indexing(ref field, ref indexing) => {

                let field_type = field_type_retriever(field.name())?;

                let indexing_type = resolve_expr(universe, scope, context, indexing)?;

                // TODO: Application?
                match indexing_type {
                    AbstractType::Int => (),

                    _ => {
                        return Err(TypeError::InvalidIndex {
                            found: indexing_type.clone(),
                            span: indexing.get_tmp(indexing.last()).span(),
                        }
                        .into());
                    }
                };

                // TODO: Application?
                match field_type {
                    AbstractType::Array {
                        element_type,
                        size: _,
                    } => {
                        next_type = *element_type;
                    }

                    _ => {
                        return Err(TypeError::NotAnArray {
                            found: field_type.clone(),
                            span: span,
                        }
                        .into());
                    }
                }
            }
        }

        current_type = next_type.clone();
    }

    let accessed_field_type = current_type;

    Ok(accessed_field_type)
}
