use std::collections::{HashMap, HashSet};

use err::{Err, TypeErr};
use ast::{DeclStmt, Function as AstFunction, Module as AstModule, Struct};
use ast::{AstNode, BuiltinFunction as AstBuiltinFunction, Ident, UseDecl};

use super::feature_checkers::*;
use super::metadata::*;
use super::smpl_type::*;
use super::semantic_data::*;
use super::semantic_data::Module;
use super::control_flow::CFG;
use super::fn_analyzer::analyze_fn;
use super::fn_type_generator::*;

use feature::*;

struct RawProgram {
    scopes: HashMap<ModuleId, ScopedData>,
    dependencies: HashMap<ModuleId, Vec<ModuleId>>,
    raw_map: HashMap<Ident, ModuleId>,
}

struct RawModData {
    name: AstNode<Ident>,
    id: ModuleId,
    reserved_structs: HashMap<Ident, ReservedType>,
    reserved_fns: HashMap<Ident, ReservedFn>,
    reserved_builtins: HashMap<Ident, ReservedBuiltinFn>,
    uses: Vec<AstNode<UseDecl>>,
}

struct ReservedType(TypeId, AstNode<Struct>);
struct ReservedFn(FnId, TypeId, AstNode<AstFunction>);
struct ReservedBuiltinFn(FnId, TypeId, AstNode<AstBuiltinFunction>);

pub fn check_modules(program: &mut Program, modules: Vec<AstModule>) -> Result<(), Err> {
    let mut raw_data = raw_mod_data(program, modules)?;

    let mut mapped_raw = HashMap::new();
    let mut scopes = HashMap::new();

    // Map reserved data
    for (mod_id, raw) in raw_data.iter() {
        let mut scope = program.universe().std_scope();
        map_internal_data(&mut scope, raw);

        mapped_raw.insert(raw.name.data().clone(), mod_id.clone());
        scopes.insert(mod_id.clone(), scope);
    }

    let mut raw_program = RawProgram {
        scopes: scopes,
        raw_map: mapped_raw,
        dependencies: HashMap::new(),
    };

    map_usings(&raw_data, &mut raw_program)?;

    let mut type_roots = Vec::new();
    // Map ALL structs into the universe before generating functions
    for (mod_id, raw_mod) in raw_data.iter() {
        for (_, reserved_type) in raw_mod.reserved_structs.iter() {
            let type_id = reserved_type.0;
            let (struct_type, field_ordering) = generate_struct_type(
                program,
                raw_program.scopes.get(mod_id).unwrap(),
                reserved_type.1.data(),
            )?;

            program
                .universe_mut()
                .insert_type(type_id, SmplType::Struct(struct_type));

            type_roots.push(type_id);

            let field_ordering = FieldOrdering::new(type_id, field_ordering);
            program
                .metadata_mut()
                .insert_field_ordering(type_id, field_ordering);
        }
    }

    for (mod_id, raw_mod) in raw_data.iter() {
        for (_, reserved_fn) in raw_mod.reserved_fns.iter() {
            let fn_id = reserved_fn.0;
            let type_id = reserved_fn.1;
            let fn_decl = reserved_fn.2.data();
            let fn_type = generate_fn_type(
                program,
                raw_program.scopes.get(mod_id).unwrap(),
                fn_id,
                reserved_fn.2.data(),
            )?;
            let cfg = CFG::generate(program.universe(), fn_decl.body.clone(), &fn_type)?;

            program
                .universe_mut()
                .insert_fn(fn_id, type_id, fn_type, cfg);
            program.metadata_mut().insert_module_fn(
                mod_id.clone(),
                fn_decl.name.data().clone(),
                fn_id,
            );
        }

        for (_, reserved_builtin) in raw_mod.reserved_builtins.iter() {
            let fn_id = reserved_builtin.0;
            let type_id = reserved_builtin.1;
            let fn_decl = reserved_builtin.2.data();
            let fn_type = generate_builtin_fn_type(
                program,
                raw_program.scopes.get(mod_id).unwrap(),
                fn_id,
                reserved_builtin.2.data(),
            )?;

            program.features_mut().add_feature(BUILTIN_FN);

            program
                .universe_mut()
                .insert_builtin_fn(fn_id, type_id, fn_type);

            program.metadata_mut().insert_builtin(fn_id);
            program.metadata_mut().insert_module_fn(
                mod_id.clone(),
                fn_decl.name.data().clone(),
                fn_id,
            );
        }
    }

    for root in type_roots.into_iter() {
        cyclic_type_check(program, root)?;
        let struct_type = program.universe().get_type(root);
        let struct_type = irmatch!(*struct_type; SmplType::Struct(ref s) => s);
        for field_type in struct_type
            .fields
            .iter()
            .map(|(_, type_id)| type_id.clone())
        {
            let (universe, _, features) = program.analysis_context();
            field_type_scanner(universe, features, field_type);
        }
    }

    for (mod_id, raw_mod) in raw_data.iter() {
        for (_, reserved_fn) in raw_mod.reserved_fns.iter() {
            let fn_id = reserved_fn.0;
            analyze_fn(
                program,
                raw_program.scopes.get(mod_id).unwrap(),
                fn_id,
                mod_id.clone(),
            )?;
        }
    }

    for (name, mod_id) in raw_program.raw_map.into_iter() {
        let module_data = raw_data.remove(&mod_id).unwrap();

        let owned_structs = module_data
            .reserved_structs
            .into_iter()
            .map(|(_, r)| r.0)
            .collect::<Vec<_>>();
        let owned_fns = module_data
            .reserved_fns
            .into_iter()
            .map(|(_, r)| r.0)
            .chain(module_data.reserved_builtins.into_iter().map(|(_, r)| r.0))
            .collect::<Vec<_>>();

        let module_scope = raw_program.scopes.remove(&mod_id).unwrap();

        let dependencies = raw_program.dependencies.remove(&mod_id).unwrap();

        let module = Module::new(module_scope, owned_structs, owned_fns, dependencies, mod_id);

        program.universe_mut().map_module(mod_id, name, module);
    }

    Ok(())
}

fn cyclic_type_check(program: &Program, root_id: TypeId) -> Result<(), Err> {
    let mut visited_structs = HashSet::new();
    let mut to_visit = Vec::new();

    to_visit.push(root_id);

    loop {
        let depth = to_visit;
        to_visit = Vec::new();
        for type_id in depth.into_iter() {
            if visited_structs.contains(&type_id) {
                return Err(TypeErr::CyclicType(root_id).into());
            }

            match *program.universe().get_type(type_id) {
                SmplType::Struct(ref struct_type) => {
                    // Remove fields with duplicate types
                    let set: HashSet<_> = struct_type
                        .fields
                        .iter()
                        .map(|(_, type_id)| type_id.clone())
                        .collect();

                    to_visit = set.into_iter().collect();

                    visited_structs.insert(type_id);
                }

                SmplType::Array(ref array_type) => {
                    to_visit.push(array_type.base_type);
                }

                _ => continue,
            }
        }

        if to_visit.len() == 0 {
            break;
        }
    }

    Ok(())
}

fn generate_struct_type(
    program: &mut Program,
    scope: &ScopedData,
    struct_def: &Struct,
) -> Result<(StructType, Vec<FieldId>), Err> {
    let (universe, _metadata, _features) = program.analysis_context();

    let mut fields = HashMap::new();
    let mut field_map = HashMap::new();
    let mut order = Vec::new();
    if let Some(ref body) = struct_def.body.0 {
        for field in body.iter() {
            let f_id = universe.new_field_id();
            let f_name = field.name.data().clone();
            let f_type_path = &field.field_type;
            let path_data = f_type_path.data();
            let field_type = scope.type_id(universe, path_data.into())?;
            fields.insert(f_id, field_type);
            field_map.insert(f_name, f_id);
            order.push(f_id);
        }
    }

    let struct_t = StructType {
        name: struct_def.name.data().clone(),
        fields: fields,
        field_map: field_map,
    };

    Ok((struct_t, order))
}

fn map_usings(
    raw_modules: &HashMap<ModuleId, RawModData>,
    raw_prog: &mut RawProgram,
) -> Result<(), Err> {
    for (id, raw_mod) in raw_modules {
        let mut dependencies = Vec::new();
        for use_decl in raw_mod.uses.iter() {
            let import_name = use_decl.data().0.data();
            let import_id = raw_prog
                .raw_map
                .get(import_name)
                .ok_or(Err::UnresolvedUses(vec![use_decl.clone()]))?;

            dependencies.push(import_id.clone());
            // Get imported module's types and functions
            let (all_types, all_fns) = {
                let imported_scope = raw_prog.scopes.get(import_id).unwrap();
                let all_types = imported_scope
                    .all_types()
                    .into_iter()
                    .map(|(path, id)| {
                        let mut path = path.clone();
                        path.0.insert(0, import_name.clone());

                        (path, id.clone())
                    })
                    .collect::<HashMap<_, _>>();
                let all_fns = imported_scope
                    .all_fns()
                    .into_iter()
                    .map(|(path, id)| {
                        let mut path = path.clone();
                        path.0.insert(0, import_name.clone());

                        (path, id.clone())
                    })
                    .collect::<HashMap<_, _>>();

                (all_types, all_fns)
            };

            let current_module_scope = raw_prog.scopes.get_mut(id).unwrap();

            // Bring imported types into scope
            for (path, imported) in all_types.into_iter() {
                if current_module_scope
                    .insert_type(path.clone().into(), imported)
                    .is_some()
                {
                    panic!("Should not have overrwritten {}. Paths should be unique by prefixing with the originating module.", path);
                }
            }

            // Bring imported functions into scope
            for (path, imported) in all_fns.into_iter() {
                current_module_scope.insert_fn(path, imported);
            }
        }

        raw_prog.dependencies.insert(id.clone(), dependencies);
    }

    Ok(())
}

fn map_internal_data(scope: &mut ScopedData, raw: &RawModData) {
    for (_ident, r) in raw.reserved_structs.iter() {
        scope.insert_type(r.1.data().name.data().clone().into(), r.0.clone());
    }

    for (_ident, r) in raw.reserved_fns.iter() {
        scope.insert_fn(r.2.data().name.data().clone().into(), r.0.clone());
    }

    for (_ident, r) in raw.reserved_builtins.iter() {
        scope.insert_fn(r.2.data().name.data().clone().into(), r.0.clone());
    }
}

fn raw_mod_data(program: &mut Program, modules: Vec<AstModule>) -> Result<HashMap<ModuleId, RawModData>, Err> {
    let universe = program.universe_mut();
    let mut mod_map = HashMap::new();
    for module in modules {
        let mut struct_reserve = HashMap::new();
        let mut fn_reserve = HashMap::new();
        let mut builtin_fn_reserve = HashMap::new();
        let mut uses = Vec::new();

        for decl_stmt in module.1.into_iter() {
            match decl_stmt {
                DeclStmt::Struct(d) => {
                    struct_reserve.insert(
                        d.data().name.data().clone().clone(),
                        ReservedType(universe.new_type_id(), d),
                    );
                }

                DeclStmt::Function(d) => {
                    fn_reserve.insert(
                        d.data().name.data().clone(),
                        ReservedFn(universe.new_fn_id(), universe.new_type_id(), d),
                    );
                }

                DeclStmt::BuiltinFunction(d) => {
                    builtin_fn_reserve.insert(
                        d.data().name.data().clone(),
                        ReservedBuiltinFn(universe.new_fn_id(), universe.new_type_id(), d),
                    );
                }

                DeclStmt::Use(u) => {
                    uses.push(u);
                }
            }
        }

        let raw = RawModData {
            name: module.0.ok_or(Err::MissingModName)?,
            id: universe.new_module_id(),
            reserved_structs: struct_reserve,
            reserved_fns: fn_reserve,
            reserved_builtins: builtin_fn_reserve,
            uses: uses,
        };

        mod_map.insert(raw.id.clone(), raw);
    }

    Ok(mod_map)
}
