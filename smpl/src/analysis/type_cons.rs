use std::collections::{HashMap, HashSet};

use crate::ast::{Struct, Ident, ModulePath, TypeAnnotation, TypeAnnotationRef};

use super::semantic_data::{FieldId, TypeId, Program, ScopedData};
use super::smpl_type::*;
use super::error::{AnalysisError, TypeError, ApplicationError};

pub enum TypeCons {

    Function { 
        type_params: Vec<TypeId>,
        parameters: Vec<TypeId>,
        return_type: TypeId,
    },

    Array {
        element_type: TypeId,
        size: u64,
    },

    Record {
        name: Ident,
        type_params: Vec<TypeId>,
        fields: HashMap<FieldId, TypeId>,
        field_map: HashMap<Ident, FieldId>,
    },

    Int,
    Float,
    String,
    Bool,
    Unit,
}

pub enum TypeArg {
    Type(TypeId),
    Number(i64),
}

impl TypeCons {
    fn apply(&self, mut args: Option<Vec<TypeArg>>) -> Result<SmplType, ApplicationError> {

        match *self {
            TypeCons::Array {
                element_type: element_type,
                size: size,
            } => {
                let mut args = args.ok_or(ApplicationError::Arity {
                    expected: 1,
                    found: 0
                })?;

                if args.len() != 1 {
                    return Err(ApplicationError::Arity {
                        expected: 1,
                        found: args.len()
                    });
                }

                let element_type = args.pop().unwrap();

                let element_type = match element_type {
                    TypeArg::Type(id) => id,
                    TypeArg::Number(_) => return Err(ApplicationError::ExpectedType { 
                        param_position: 0
                    }),
                };

                let array_type = ArrayType { 
                    base_type: element_type,
                    size: size as u64
                };

                Ok(SmplType::Array(array_type))
            }

            TypeCons::Record {
                name: ref name,
                type_params: ref type_params,
                fields: ref fields,
                field_map: ref field_map,
            } => {
                if type_params.len() > 0 {
                    let mut args = args.ok_or(ApplicationError::Arity {
                        expected: type_params.len(),
                        found: 0
                    })?;

                    if args.len() != type_params.len() {
                        return Err(ApplicationError::Arity {
                            expected: type_params.len(),
                            found: args.len()
                        });
                    }

                    // Map parameters to arguments
                    let mut param_to_arg_map = HashMap::new();
                    for (pos, (param, arg)) in type_params.iter().zip(args).enumerate() {
                        match arg {
                            TypeArg::Type(id) => param_to_arg_map.insert(param, id),

                            TypeArg::Number(_) => {
                                return Err(ApplicationError::ExpectedType {
                                    param_position: pos
                                });
                            }
                        };
                    }

                    // Apply type arguments to the record type
                    let applied_fields = fields
                        .iter()
                        .map(|(field_id, field_type_id)| {

                            let type_id = param_to_arg_map.get(field_type_id)
                                .unwrap_or(field_type_id);

                            (field_id.clone(), type_id.clone())
                        })
                    .collect::<HashMap<_,_>>();

                    let struct_type = StructType {
                        name: name.clone(),
                        fields: applied_fields.clone(),
                        field_map: field_map.clone(),
                    };

                    Ok(SmplType::Struct(struct_type))

                } else {
                    if let Some(args) = args {
                        Err(ApplicationError::Arity {
                            expected: 0,
                            found: args.len(),
                        })
                    } else {
                        let struct_type = StructType {
                            name: name.clone(),
                            fields: fields.clone(),
                            field_map: field_map.clone(),
                        };

                        Ok(SmplType::Struct(struct_type))
                    }
                }
            }

            TypeCons::Function {
                    type_params: ref type_params,
                    parameters: ref params,
                    return_type: ref return_type,
            } => {
                if type_params.len() > 0 {
                    let mut args = args.ok_or(ApplicationError::Arity {
                        expected: type_params.len(),
                        found: 0
                    })?;

                    if args.len() != type_params.len() {
                        return Err(ApplicationError::Arity {
                            expected: type_params.len(),
                            found: args.len()
                        });
                    }

                    // Map parameters to arguments
                    let mut param_to_arg_map = HashMap::new();
                    for (pos, (param, arg)) in type_params.iter().zip(args).enumerate() {
                        match arg {
                            TypeArg::Type(id) => param_to_arg_map.insert(param, id),

                            TypeArg::Number(_) => {
                                return Err(ApplicationError::ExpectedType {
                                    param_position: pos
                                });
                            }
                        };
                    }

                    // Apply type arguments to the function_type type
                    let applied_params = params
                        .iter()
                        .map(|param_type_id| {
                            param_to_arg_map.get(param_type_id)
                                .unwrap_or(param_type_id)
                                .clone()
                        })
                    .collect::<Vec<_>>();

                    let applied_return = param_to_arg_map.get(return_type)
                        .unwrap_or(return_type);

                    let param_type = ParamType::Checked(applied_params);
                    let fn_type = FunctionType {
                        params: param_type,
                        return_type: applied_return.clone(),
                    };

                    Ok(SmplType::Function(fn_type))

                } else {
                    if let Some(args) = args {
                        Err(ApplicationError::Arity {
                            expected: 0,
                            found: args.len(),
                        })
                    } else {
                        let param_type = ParamType::Checked(params.clone());
                        let fn_type = FunctionType {
                            params: param_type,
                            return_type: return_type.clone(),
                        };

                        Ok(SmplType::Function(fn_type))
                    }
                }

            }

            ref tc @ TypeCons::Int  | 
                ref tc @ TypeCons::Float | 
                ref tc @ TypeCons::String | 
                ref tc @ TypeCons::Bool | 
                ref tc @ TypeCons::Unit => {
            
                if let Some(args) = args {
                    return Err(ApplicationError::Arity {
                        expected: 0,
                        found: args.len(),
                    });
                } else {
                    let t = match tc {
                        TypeCons::Int => SmplType::Int,
                        TypeCons::Float => SmplType::Float,
                        TypeCons::String => SmplType::String,
                        TypeCons::Bool => SmplType::Bool,
                        TypeCons::Unit => SmplType::Unit,

                        _ => unreachable!(),
                    };

                    Ok(t)
                }
            }
        }
    }
}

pub fn type_cons_from_annotation<'a, 'b, 'c, 'd, T: Into<TypeAnnotationRef<'c>>>(
    program: &'a mut Program,
    scope: &'b ScopedData,
    anno: T,
    type_params: &'d HashMap<&'d Ident, TypeId>
    ) -> Result<TypeId, AnalysisError> {

    match anno.into() {
        TypeAnnotationRef::Path(typed_path) => {
            if typed_path.module_path().0.len() == 1 {
                // Check if path refers to type parameter
            } else {
                // Get type id from Scope
            }

            unimplemented!()
        },

        TypeAnnotationRef::Array(element_type, size) => {
            let element_type_cons = type_cons_from_annotation(program,
                                                              scope,
                                                              element_type.data(),
                                                              type_params)?;
            let cons = TypeCons::Array {
                element_type: element_type_cons,
                size: size.clone(),
            };

            // TODO: Insert type constructor in the universe
            unimplemented!()
        },

        TypeAnnotationRef::FnType(args, ret_type) => {

            let arg_type_cons = match args.map(|slice|{
                slice.iter().map(|arg| type_cons_from_annotation(program,
                                                                 scope,
                                                                 arg.data(),
                                                                 type_params)
                                 )
                    .collect::<Result<Vec<_>, _>>()
            }) {
                Some(args) => Some(args?),
                None => None,
            };

            let return_type_cons = match ret_type.map(|ret_type| {
                type_cons_from_annotation(program,
                                          scope,
                                          ret_type.data(),
                                          type_params)
            }) {
                Some(ret) => Some(ret?),
                None => None,
            };

            let cons = TypeCons::Function {
                type_params: type_params
                    .values()
                    .map(|type_id| type_id.clone())
                    .collect(),
                parameters: arg_type_cons.unwrap_or(Vec::new()),
                return_type: return_type_cons.unwrap_or(program.universe().unit()),
            };

            // TODO: Insert type constructor in the universe
            unimplemented!()
        },
    }

}

// TODO: Store type constructors in Program
pub fn generate_struct_type_cons(
    program: &mut Program,
    scope: &ScopedData,
    struct_def: &Struct,
) -> Result<Vec<FieldId>, AnalysisError> {
    let (universe, _metadata, _features) = program.analysis_context();

    // Check no parameter naming conflicts
    let mut type_parameter_map = HashMap::new();
    match struct_def.type_params {
        Some(ref params) => {
            for p in params.params.iter() {
                if type_parameter_map.contains_key(p.data()) {
                    return Err(TypeError::ParameterNamingConflict {
                        ident: p.data().clone(),
                    }.into());
                } else {
                    type_parameter_map.insert(p.data(), universe.new_type_id());
                }
            }
        }

        None => (),
    }

    // Generate the constructor
    let mut fields = HashMap::new();
    let mut field_map = HashMap::new();
    let mut order = Vec::new();
    if let Some(ref body) = struct_def.body.0 {
        for field in body.iter() {
            let f_id = universe.new_field_id();
            let f_name = field.name.data().clone();
            let f_type_path = &field.field_type;
            let path_data = f_type_path.data();

            let field_type = match struct_def.type_params {

                // TODO: Recursively generate type constructor generators for fields
                Some(ref params) => {
                    unimplemented!()
                }

                // No type parameters, behave as if concrete type constructor
                None => scope.type_id(universe, path_data.into())?,
            };

            // Map field to type constructor
            fields.insert(f_id, field_type);

            if field_map.contains_key(&f_name) {
                return Err(TypeError::FieldNamingConflict {
                    ident: f_name.clone(),
                }.into());
            } else {
                field_map.insert(f_name, f_id);
            }

            order.push(f_id);
        }
    }

    // TODO: Insert type constructor
    let type_cons = TypeCons::Record {
        name: struct_def.name.data().clone(),
        fields: fields,
        field_map: field_map,
        type_params: type_parameter_map
            .values()
            .map(|id| id.clone())
            .collect(),
    };
    

    Ok(order)
}
