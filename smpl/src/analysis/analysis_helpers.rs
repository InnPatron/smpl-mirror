use std::collections::HashMap;

use crate::ast;
use crate::span::Span;

use super::error::AnalysisError;
use super::metadata::*;
use super::resolve_scope::ScopedData;
use super::semantic_data::{
    FieldId, FnId, Program, TypeId, TypeParamId, TypeVarId,
    Universe, ModuleId, Function,
};
use super::type_checker::TypingContext;
use super::type_cons::{TypeCons, TypeParams};
use super::abstract_type::AbstractType;
use super::analysis_context::{GlobalData, LocalData, AnalysisContext};
use super::anon_storage::AnonStorage;

pub fn analyze_fn(
    universe: &mut Universe,
    metadata: &mut Metadata,
    global_data: &mut GlobalData,
    module_id: ModuleId,
    fn_id: FnId,
) -> Result<(), AnalysisError> {
    use super::resolve_scope;
    use super::return_trace;
    use super::type_checker;

    resolve_scope::resolve(universe, fn_id)?;
    type_checker::type_check(universe, metadata, global_data, module_id, fn_id)?;
    return_trace::return_trace(universe, fn_id)?;

    Ok(())
}

pub fn analyze_fn_prime(
    to_analyze: &mut Function,
    universe: &Universe,
    metadata: &mut Metadata,
    global_data: &mut GlobalData,
    local_data: &mut LocalData,
    module_id: ModuleId,
) -> Result<(), AnalysisError> {
    use super::resolve_scope;
    use super::return_trace;
    use super::type_checker;

    let anon_scopes: AnonStorage<ScopedData> =
        resolve_scope::resolve_prime(to_analyze)?;

    let (anon_typing_contexts, anon_type_cons): (AnonStorage<TypingContext>, AnonStorage<TypeCons>) =
        type_checker::type_check_prime(
            to_analyze,
            universe,
            metadata,
            global_data,
            module_id,
        )?;

    let _ = return_trace::return_trace_prime(to_analyze)?;

    Ok(())
}

pub struct ContextData<'a> {
    type_params: Option<&'a ast::TypeParams>,
    params: Option<&'a [ast::AstNode<ast::FnParameter>]>,
    clear_variables: bool,
}

impl<'a> From<&'a ast::Function> for ContextData<'a> {
    fn from(ast_fn: &ast::Function) -> ContextData {
        ContextData {
            type_params: ast_fn.type_params.as_ref(),
            params: ast_fn.params.as_ref().map(|v| v.as_slice()),
            clear_variables: true,
        }
    }
}

impl<'a> From<&'a ast::AnonymousFn> for ContextData<'a> {
    fn from(ast_fn: &ast::AnonymousFn) -> ContextData {
        ContextData {
            type_params: None,
            params: ast_fn.params.as_ref().map(|v| v.as_slice()),
            clear_variables: true,
        }
    }
}

pub fn generate_fn_analysis_data<'a, 'b, 'c, 'd, 'e, T>(
    universe: &'a Universe,
    global_data: &'a mut GlobalData,
    local_data: &'a mut LocalData,
    outer_scope: &'b ScopedData,
    outer_context: &'c TypingContext,
    fn_type_cons: &'d TypeCons,
    fn_def: &'e T,
) -> Result<AnalysisContext, AnalysisError>
where
    &'e T: Into<ContextData<'e>>,
{
    let fn_def: ContextData = fn_def.into();

    let mut fn_scope = outer_scope.clone();
    let mut fn_context = outer_context.clone();
    let mut existential_type_vars = Vec::new();

    if fn_def.clear_variables {
        fn_scope.clear_scoped_vars();
    }

    match fn_type_cons {
        TypeCons::Function {
            ref type_params,
            ref parameters,
            ..
        } => {
            let mut existential_map = HashMap::new();
            // Map placeholder type variables to an existential type variable
            if let Some(ref tps) = fn_def.type_params {
                for (param_name, (type_param_id, constraint)) in
                    tps.params.iter().zip(type_params.iter())
                {
                    let existential_type_var = global_data.new_type_var_id();
                    let placeholder_variable =
                        type_params.placeholder_type_var(type_param_id);

                    fn_scope.insert_type_var(
                        param_name.data().clone(),
                        existential_type_var,
                    );

                    existential_map.insert(
                        placeholder_variable,
                        AbstractType::TypeVar(constraint.span().clone(),
                            existential_type_var),
                    );

                    existential_type_vars.push(existential_type_var);

                    // Constraint guarenteed to be AbstractType::WidthConstraint or
                    //   AbstractType::Any by TypeParams
                    fn_context
                        .type_vars
                        .insert(existential_type_var, constraint.clone());
                }
            }

            // Map formal parameters into scope and typing context
            if let Some(ref formal_params) = fn_def.params {
                for (formal_param, formal_param_type) in
                    formal_params.iter().zip(parameters.iter())
                {
                    let formal_param_var_id = local_data.new_var_id();
                    let formal_param_type = formal_param_type.substitute_with(
                        universe,
                        outer_scope,
                        outer_context,
                        &existential_map,
                    )?;

                    fn_scope.insert_var(
                        formal_param.data().name.data().clone(),
                        formal_param_var_id,
                    );

                    fn_context
                        .var_type_map
                        .insert(formal_param_var_id, formal_param_type);
                }
            }
        }

        _ => unreachable!("Only pass in a function type constructor"),
    }

    Ok(AnalysisContext::new(
        fn_scope,
        fn_context,
        existential_type_vars,
    ))
}
