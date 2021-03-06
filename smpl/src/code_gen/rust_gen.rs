use crate::code_gen::StringEmitter;

use petgraph::graph::NodeIndex;

use crate::feature::*;

use crate::ast::Ident;

use crate::analysis::*;
use crate::analysis::smpl_type::*;
use crate::analysis::metadata::*;

pub struct RustBackend {
    mods: Vec<(Ident, ModuleId, String)>,
    main: Option<String>,
    mod_wrap: bool,
    feature_info: FeatureInfo,
}

impl RustBackend {
    fn features() -> FeatureInfo {
        let required = Vec::new();
        let mut denied = Vec::new();

        denied.push(FeatureReasoning::with_feature(MOD_ACCESS));
        denied.push(FeatureReasoning::with_feature(FUNCTION_VALUE));
        denied.push(FeatureReasoning::with_feature(BUILTIN_FN));
        denied.push(FeatureReasoning::with_feature(UNCHECKED_BUILTIN_FN_PARAMS));
        denied.push(FeatureReasoning::with_feature(ANONYMOUS_FN));

        FeatureInfo::new(required, denied)
    }

    pub fn new() -> RustBackend {
        RustBackend {
            mods: Vec::new(),
            main: None,
            mod_wrap: false,

            feature_info: RustBackend::features(),
        }
    }

    pub fn main(&self) -> Option<&String> {
        self.main.as_ref()
    }

    pub fn wrap_mod(mut self) -> RustBackend {
        self.mod_wrap = true;
        self
    }

    pub fn generate(mut self, program: &Program) -> Result<RustBackend, FeatureErr> {
        self.feature_info.check(program.features())?;

        self.main = program.metadata().main().map(|(fn_id, mod_id)| {
            if self.mod_wrap {
                format!(
                    "fn main() {{ {}::{}(); }}\n\n",
                    RustGenFmt::mod_id(mod_id),
                    RustGenFmt::fn_id(fn_id)
                )
            } else {
                format!("fn main() {{ {}(); }}\n\n", RustGenFmt::fn_id(fn_id))
            }
        });

        for &(ident, id) in program.universe().all_modules().iter() {
            let mut gen = RustModGen::new(program.metadata(), *id);

            if self.mod_wrap {
                gen.output
                    .emit_line(&format!("mod {} {{", RustGenFmt::mod_id(*id)));
                gen.output.shift_right();
            }

            gen.emit_mod(program.universe(), program.universe().get_module(*id));

            if self.mod_wrap {
                gen.output.shift_left();
                gen.output.emit_line("}");
            }

            self.mods
                .push((ident.clone(), id.clone(), gen.module().to_string()));
        }

        Ok(self)
    }

    pub fn finalize(self) -> Vec<(Ident, ModuleId, String)> {
        self.mods
    }
}

struct RustModGen<'a> {
    output: StringEmitter,
    metadata: &'a Metadata,
    id: ModuleId,
}

impl<'a> RustModGen<'a> {
    fn new(metadata: &Metadata, id: ModuleId) -> RustModGen {
        RustModGen {
            output: StringEmitter::new(),
            metadata: metadata,
            id: id,
        }
    }

    fn module(&self) -> &str {
        self.output.output()
    }

    fn emit_mod(&mut self, universe: &Universe, module: &Module) {
        self.prelude(universe, module);

        self.output
            .emit_line("//#### START STRUCT DEFINITIONS ####");

        // Emit struct definitions
        for id in module.owned_types() {
            let id = *id;
            let t = universe.get_type(id);
            if let SmplType::Struct(ref struct_t) = *t {
                self.emit_struct_type(id, struct_t);
                self.output.line_pad();
            }
        }

        self.output.emit_line("//#### END  STRUCT DEFINITIONS ####");
        self.output.line_pad();
        self.output
            .emit_line("//#### START FUNCTION DEFINITIONS ####");

        // Emit function definitions
        for fn_id in module.owned_fns() {
            let fn_id = *fn_id;
            let func = universe.get_fn(fn_id);
            let func_type = universe.get_type(func.type_id());
            let name = RustGenFmt::fn_id(fn_id);

            let return_type_id;
            let mut args = String::new();
            if let SmplType::Function(ref fn_type) = *func_type {
                // Get return type
                return_type_id = fn_type.return_type;

                // Gather parameters
                // TODO: Support UNCHECKED functions
                let params = irmatch!(&fn_type.params; ParamType::Checked(ref params) => params);
                for (param_type_id, param_var_id) in params
                    .iter()
                    .zip(self.metadata.function_param_ids(fn_id).iter())
                {
                    let param_type = RustGenFmt::type_id(*param_type_id);
                    let param_type = RustGenFmt::rustify_type(param_type);

                    args.push_str(&format!(
                        "{}: {}, ",
                        RustGenFmt::var_id(param_var_id.var_id()),
                        param_type
                    ));
                }
            } else {
                panic!("{} did not map to a function type", func.type_id());
            }
            let return_type = RustGenFmt::type_id(return_type_id);
            let return_type = RustGenFmt::rustify_type(return_type);

            // Emit fn signature
            match *universe.get_type(return_type_id) {
                SmplType::Unit => {
                    self.output.emit(&format!("pub fn {} ({})", name, args));
                }

                _ => {
                    self.output.emit(&format!("pub fn {} (", name));
                    self.output.emit(&args);
                    self.output.emit(&format!(") -> {} ", return_type));
                }
            }

            // Emit CFG
            let mut cfg = func.cfg();
            let mut fn_gen = RustFnGen::new(universe, &cfg);

            {
                let traverser = Traverser::new(&*cfg, &mut fn_gen);
                traverser.traverse().unwrap();
            }

            let fn_string = fn_gen.function();
            self.output.emit(&fn_string);

            self.output.line_pad();
        }
        self.output
            .emit_line("//#### END FUNCTION DEFINITIONS ####");
    }

    fn prelude(&mut self, universe: &Universe, module: &Module) {
        self.output.emit_line("use std::cell::RefCell;");
        self.output.emit_line("use std::rc::Rc;");
        self.output.line_pad();

        // Import other SMPL modules
        for d in module.dependencies() {
            self.output
                .emit_line(&format!("use {}::*;", RustGenFmt::mod_id(*d)));
        }
        self.output.line_pad();

        // Create TypeId aliases for primitives
        self.output.emit_line(&format!(
            "type {} = ();",
            RustGenFmt::type_id(universe.unit())
        ));
        self.output.emit_line(&format!(
            "type {} = i64;",
            RustGenFmt::type_id(universe.int())
        ));

        self.output.emit_line(&format!(
            "type {} = f64;",
            RustGenFmt::type_id(universe.float())
        ));

        self.output.emit_line(&format!(
            "type {} = bool;",
            RustGenFmt::type_id(universe.boolean())
        ));

        self.output.emit_line(&format!(
            "type {} = String;",
            RustGenFmt::type_id(universe.string())
        ));

        if let Some(v) = self.metadata.array_types(self.id) {
            for t in v {
                match *universe.get_type(*t) {
                    SmplType::Array(ref a) => {
                        self.output.emit_line(&format!(
                            "type {} = [{}; {}];",
                            RustGenFmt::type_id(*t),
                            RustGenFmt::rustify_type(RustGenFmt::type_id(a.base_type)),
                            a.size
                        ));
                    }

                    _ => unreachable!(),
                }
            }
        }

        self.output.line_pad();
    }

    fn emit_struct_type(&mut self, id: TypeId, struct_type: &StructType) {
        let name = RustGenFmt::type_id(id);
        let fields = struct_type
            .fields
            .iter()
            .map(|(name, id)| (name.clone(), RustGenFmt::type_id(*id)))
            .collect::<Vec<_>>();

        self.output.emit_line("#[derive(Debug, PartialEq)]");
        self.output.emit_line(&format!("pub struct {} {{", name));
        self.output.shift_right();
        for &(ref id, ref string_type) in fields.iter() {
            let name = RustGenFmt::field_id(*id);
            let field_type = RustGenFmt::rustify_type(string_type.to_string());
            self.output
                .emit_line(&format!("pub {}: {},", name, field_type));
        }
        self.output.shift_left();
        self.output.emit_line("}");

        self.output.line_pad();

        // Emit Clone impl
        self.output
            .emit_line(&format!("impl Clone for {} {{", name));
        self.output.shift_right();
        self.output
            .emit_line(&format!("fn clone(&self) -> Self {{"));

        self.output.shift_right();
        self.output.emit_line(&format!("{} {{", name));
        self.output.shift_right();
        for &(ref name, _) in fields.iter() {
            let name = RustGenFmt::field_id(*name);
            let value = RustGenFmt::new_value("Default::default()".to_string());
            self.output.emit_line(&format!("{}: {},", name, value));
        }
        self.output.emit_line("}");
        self.output.shift_left();
        self.output.shift_left();

        self.output.emit_line("}");
        self.output.shift_left();
        self.output.emit_line("}");

        // Emit Default impl
        self.output
            .emit_line(&format!("impl Default for {} {{", name));
        self.output.shift_right();
        self.output.emit_line(&format!("fn default() -> Self {{"));

        self.output.shift_right();
        self.output.emit_line(&format!("{} {{", name));
        self.output.shift_right();
        for &(ref name, _) in fields.iter() {
            let name = RustGenFmt::field_id(*name);
            let value = RustGenFmt::new_value("Default::default()".to_string());
            self.output.emit_line(&format!("{}: {},", name, value));
        }
        self.output.emit_line("}");
        self.output.shift_left();
        self.output.shift_left();

        self.output.emit_line("}");
        self.output.shift_left();
        self.output.emit_line("}");
    }
}

pub struct RustFnGen<'a> {
    output: StringEmitter,
    cfg: &'a CFG,
    universe: &'a Universe,
}

// Misc
impl<'a> RustFnGen<'a> {
    pub fn new(universe: &'a Universe, cfg: &'a CFG) -> RustFnGen<'a> {
        RustFnGen {
            output: StringEmitter::new(),
            cfg: cfg,
            universe: universe,
        }
    }

    pub fn function(&self) -> &str {
        self.output.output()
    }
}

// Code generation
impl<'a> RustFnGen<'a> {
    fn emit_condition(&mut self, e: &Expr) {
        self.output.emit_line("if {");
        self.output.shift_right();
        let expr = self.emit_expr(e);
        self.output
            .emit_line(&format!("{} }}", RustGenFmt::tmp_id(expr)));
        self.output.shift_left();
    }

    fn emit_expr(&mut self, expr: &Expr) -> TmpId {
        let execution_order = expr.execution_order();

        let mut last_tmp = None;
        for tmp in execution_order {
            self.emit_tmp(expr.get_tmp(*tmp));
            last_tmp = Some(tmp);
        }

        *last_tmp.unwrap()
    }

    fn emit_tmp(&mut self, tmp: &Tmp) {
        let id = tmp.id();
        let value = tmp.value();

        let lhs = RustGenFmt::tmp_id(id);

        self.output.emit_line(&format!("let {} = {{", lhs));
        match *value.data() {
            Value::Literal(ref lit) => match *lit {
                Literal::String(ref string) => {
                    let lit = format!("\"{}\".to_string()", string);
                    self.output.emit_line(&lit);
                }

                Literal::Int(int) => self.output.emit_line(&int.to_string()),

                Literal::Float(float) => self.output.emit_line(&float.to_string()),

                Literal::Bool(boolean) => self.output.emit_line(&boolean.to_string()),
            },

            Value::Binding(ref var) => {
                let value = match var.get_id()
                    .expect("If the program passed semantic analysis, all IDs should be filled in.")
                {
                    BindingId::Fn(id) => RustGenFmt::fn_id(id),
                    BindingId::Var(id) => RustGenFmt::var_id(id),
                };
                let inner = RustGenFmt::borrow(value);
                self.output.emit_line(&RustGenFmt::clone_value(inner));
            }

            Value::FieldAccess(ref access) => {
                self.output.emit_line("{");

                let path = access.path();
                let root_var_id = path.root_var_id();
                let root_var = RustGenFmt::var_id(root_var_id);

                self.output.emit_line(&format!(
                    "let _borrow_{} = {};",
                    root_var,
                    RustGenFmt::borrow_ref(root_var.clone())
                ));
                let mut previous_borrow = root_var;

                if let Some(tmp) = path.root_indexing_expr() {
                    let tmp = RustGenFmt::tmp_id(tmp);

                    self.output.emit_line(&format!(
                        "let _borrow_{} = _borrow{}[{}].borrow();",
                        tmp, previous_borrow, tmp
                    ));
                    previous_borrow = tmp;
                }

                for ps in path.path() {
                    match *ps {
                        PathSegment::Ident(ref f) => {
                            // Borrow field
                            let stringified_field = RustGenFmt::field_id(f.field_id());
                            let borrow = RustGenFmt::borrow_ref(format!(
                                "_borrow_{}.{}",
                                previous_borrow, stringified_field
                            ));

                            self.output.emit_line(&format!(
                                "let _borrow_{} = {};",
                                stringified_field, borrow
                            ));

                            previous_borrow = stringified_field;
                        }

                        PathSegment::Indexing(ref f, ref tmp) => {
                            // Borrow field
                            let stringified_field = RustGenFmt::field_id(f.field_id());
                            let borrow = RustGenFmt::borrow_ref(format!(
                                "_borrow_{}.{}",
                                previous_borrow, stringified_field
                            ));

                            self.output.emit_line(&format!(
                                "let _borrow_{} = {};",
                                stringified_field, borrow
                            ));

                            let tmp = RustGenFmt::tmp_id(*tmp);

                            self.output.emit_line(&format!(
                                "let _borrow_{} = _borrow_{}[{}].borrow();",
                                tmp, stringified_field, tmp
                            ));

                            previous_borrow = tmp;
                        }
                    }
                }

                // Clone Ref inner value. TMPs are ALWAYS raw values.
                let value = RustGenFmt::clone_value(format!("_borrow_{}", previous_borrow));

                self.output.emit_line(&value);
                self.output.emit_line("}");
            }

            Value::BinExpr(ref op, ref lhs, ref rhs) => {
                self.output.emit_line(&format!(
                    "{} {} {}",
                    RustGenFmt::tmp_id(*lhs.data()),
                    RustGenFmt::bin_op(op),
                    RustGenFmt::tmp_id(*rhs.data())
                ));
            }

            Value::UniExpr(ref op, ref tmp) => {
                self.output.emit_line(&format!(
                    "{}{}",
                    RustGenFmt::uni_op(op),
                    RustGenFmt::tmp_id(*tmp.data())
                ));
            }

            Value::FnCall(ref fn_call) => {
                let binding_id = fn_call.get_id().unwrap();

                // Gather argument expressions
                let mut arg_string = String::new();
                match fn_call.args() {
                    Some(ref args) => for a in args.iter() {
                        let arg = RustGenFmt::tmp_id(*a.data());
                        arg_string.push_str(&format!("{}, ", RustGenFmt::new_value(arg)));
                    },

                    None => (),
                }

                match binding_id {
                    BindingId::Var(..) => unreachable!(),
                    BindingId::Fn(fn_id) => {
                        self.output.emit_line(&format!(
                            "{}({})",
                            RustGenFmt::fn_id(fn_id),
                            arg_string
                        ));
                    }
                }
            }

            Value::StructInit(ref struct_init) => {
                let struct_id = struct_init.struct_type().unwrap();

                let mut field_init = String::new();
                match struct_init.field_init() {
                    Some(init_list) => for (ref field, ref typed_tmp) in init_list {
                        field_init.push_str(&format!(
                            "{}: {},",
                            RustGenFmt::field_id(*field),
                            RustGenFmt::new_value(RustGenFmt::tmp_id(*typed_tmp.data())),
                        ));
                    },

                    None => (),
                }

                self.output.emit_line(&format!(
                    "{} {{ {} }}",
                    RustGenFmt::type_id(struct_id),
                    field_init
                ));
            }

            Value::ArrayInit(ref a) => {
                let t = match *a {
                    ArrayInit::List(ref vec) => {
                        let mut string_buffer = "[".to_string();
                        for tmp in vec.iter() {
                            let value = RustGenFmt::new_value(RustGenFmt::tmp_id(*tmp.data()));
                            string_buffer.push_str(&value);
                            string_buffer.push(',');
                        }

                        string_buffer.push(']');

                        string_buffer
                    }

                    ArrayInit::Value(ref value, size) => {
                        let mut string_buffer = "[".to_string();
                        let value = RustGenFmt::new_value(RustGenFmt::tmp_id(*value.data()));
                        for _ in 0..size {
                            string_buffer.push_str(&value);
                            string_buffer.push(',');
                        }

                        string_buffer.push(']');

                        string_buffer
                    }
                };

                self.output.emit_line(&t);
            }

            Value::Indexing(ref indexing) => {
                let array_tmp_id = *indexing.array.data();
                let indexer_tmp_id = *indexing.indexer.data();

                self.output.emit_line("{");
                self.output.emit_line(&format!(
                    "let _borrow_{} = {};",
                    RustGenFmt::tmp_id(array_tmp_id),
                    RustGenFmt::tmp_id(array_tmp_id)
                ));

                self.output.emit_line(&format!(
                    "let _borrow_{} = _borrow_{}[{}].borrow();",
                    RustGenFmt::tmp_id(indexer_tmp_id),
                    RustGenFmt::tmp_id(array_tmp_id),
                    RustGenFmt::tmp_id(indexer_tmp_id)
                ));
                let value = RustGenFmt::clone_value(format!(
                    "_borrow_{}",
                    RustGenFmt::tmp_id(indexer_tmp_id)
                ));
                self.output.emit_line(&value);

                self.output.emit_line("}");
            }

            Value::ModAccess(_) => panic!(),

            Value::AnonymousFn(_) => panic!(),
        }

        self.output.emit_line("};");
    }
}

impl<'a> Passenger<()> for RustFnGen<'a> {
    fn start(&mut self, _id: NodeIndex) -> Result<(), ()> {
        // Emit nothing
        Ok(())
    }

    fn end(&mut self, _id: NodeIndex) -> Result<(), ()> {
        // Emit nothing
        Ok(())
    }

    fn branch_split(&mut self, _id: NodeIndex, _: &BranchingData) -> Result<(), ()> {
        // Emit nothing
        Ok(())
    }

    fn branch_merge(&mut self, _id: NodeIndex, _: &BranchingData) -> Result<(), ()> {
        // Emit nothing
        Ok(())
    }

    fn loop_head(&mut self, _id: NodeIndex, _: &LoopData) -> Result<(), ()> {
        self.output.emit_line("loop {");
        self.output.shift_right();
        Ok(())
    }

    fn loop_foot(&mut self, _id: NodeIndex, _: &LoopData) -> Result<(), ()> {
        /*
        if self.previous_is_loop_head {
            self.output.emit_line("{ break; }");
            self.previous_is_loop_head = false;
        }
        // Can't be used; is it necessary?
        */

        self.output.shift_left();
        self.output.emit_line("}");
        self.output.line_pad();
        Ok(())
    }

    fn cont(&mut self, _id: NodeIndex, _: &LoopData) -> Result<(), ()> {
        self.output.emit_line("continue;");
        self.output.line_pad();
        Ok(())
    }

    fn br(&mut self, _id: NodeIndex, _: &LoopData) -> Result<(), ()> {
        self.output.emit_line("break;");
        self.output.line_pad();
        Ok(())
    }

    fn enter_scope(&mut self, _id: NodeIndex) -> Result<(), ()> {
        self.output.emit_line("{");
        self.output.shift_right();
        Ok(())
    }

    fn exit_scope(&mut self, _id: NodeIndex) -> Result<(), ()> {
        self.output.shift_left();
        self.output.emit_line("}");
        Ok(())
    }

    fn local_var_decl(&mut self, _id: NodeIndex, var_decl: &LocalVarDeclData) -> Result<(), ()> {
        let var_decl = &var_decl.decl;
        let var_id = var_decl.var_id();
        let type_id = var_decl.type_id().unwrap();
        let expr = self.emit_expr(var_decl.init_expr());

        let name = RustGenFmt::var_id(var_id);
        let var_type = RustGenFmt::type_id(type_id);
        let expr = RustGenFmt::new_value(RustGenFmt::tmp_id(expr));

        let var_type = RustGenFmt::rustify_type(var_type);

        self.output
            .emit_line(&format!("let mut {}: {} = {};", name, var_type, expr));
        self.output.line_pad();

        Ok(())
    }

    fn assignment(&mut self, _id: NodeIndex, assignment: &AssignmentData) -> Result<(), ()> {
        let assignment = &assignment.assignment;
        let path = assignment.assignee().path();
        let root_var_id = path.root_var_id();
        let root_var = RustGenFmt::var_id(root_var_id);

        self.output.emit_line("{");
        self.output.shift_right();

        if assignment.access().order_length() > 0 {
            // NOTE: Don't need to do anything with the assignment access expression except emit it
            // Will be used by the root
            self.emit_expr(assignment.access());
        }
        let value_expr = self.emit_expr(assignment.value());

        self.output.emit_line(&format!(
            "let mut _borrow_{} = {};",
            root_var,
            RustGenFmt::borrow_ref_mut(root_var.clone())
        ));

        let mut previous_borrow = root_var;

        if let Some(tmp) = path.root_indexing_expr() {
            let tmp = RustGenFmt::tmp_id(tmp);

            self.output.emit_line(&format!(
                "let mut _borrow_{} = _borrow_{}[{}].borrow_mut();",
                tmp, previous_borrow, tmp
            ));

            previous_borrow = tmp;
        }

        for ps in path.path() {
            match *ps {
                PathSegment::Ident(ref f) => {
                    // Borrow field
                    let stringified_field = RustGenFmt::field_id(f.field_id());
                    let borrow = RustGenFmt::borrow_ref_mut(format!(
                        "_borrow_{}.{}",
                        previous_borrow, stringified_field
                    ));

                    self.output.emit_line(&format!(
                        "let mut _borrow_{} = {};",
                        stringified_field, borrow
                    ));

                    previous_borrow = stringified_field;
                }

                PathSegment::Indexing(ref f, ref tmp) => {
                    // Borrow field
                    let stringified_field = RustGenFmt::field_id(f.field_id());
                    let borrow = RustGenFmt::borrow_ref(format!(
                        "_borrow_{}.{}",
                        previous_borrow, stringified_field
                    ));

                    self.output.emit_line(&format!(
                        "let mut _borrow_{} = {};",
                        stringified_field, borrow
                    ));

                    let tmp = RustGenFmt::tmp_id(*tmp);

                    self.output.emit_line(&format!(
                        "let mut _borrow_{} = _borrow_{}[{}].borrow_mut();",
                        tmp, stringified_field, tmp
                    ));

                    previous_borrow = tmp;
                }
            }
        }

        let assignee = format!("*_borrow_{}", previous_borrow);
        let rhs = {
            let tmp = RustGenFmt::tmp_id(value_expr);
            tmp
        };

        let result = format!("{} = {};", assignee, rhs);
        self.output.emit_line(&result);
        self.output.shift_left();
        self.output.emit_line("}");
        self.output.line_pad();

        Ok(())
    }

    fn expr(&mut self, _id: NodeIndex, expr: &ExprData) -> Result<(), ()> {
        let expr = &expr.expr;
        self.emit_expr(expr);
        self.output.line_pad();
        Ok(())
    }

    fn ret(&mut self, _id: NodeIndex, r_data: &ReturnData) -> Result<(), ()> {
        let expr = &r_data.expr;
        match expr {
            Some(ref expr) => {
                let expr = self.emit_expr(expr);
                self.output
                    .emit_line(&format!("return {};", RustGenFmt::tmp_id(expr)));
            }

            None => self.output.emit_line("return;"),
        }

        self.output.line_pad();
        Ok(())
    }

    fn loop_condition(&mut self, _id: NodeIndex, condition_expr: &ExprData) -> Result<(), ()> {
        let condition_expr = &condition_expr.expr;
        self.emit_condition(condition_expr);

        Ok(())
    }

    fn loop_start_true_path(&mut self, _id: NodeIndex) -> Result<(), ()> {
        Ok(())
    }

    fn loop_end_true_path(&mut self, _id: NodeIndex) -> Result<(), ()> {
        self.output.emit_line("else { break; }");
        self.output.shift_left();
        self.output.emit_line("}");
        Ok(())
    }

    fn branch_condition(&mut self, _id: NodeIndex, condition_expr: &ExprData) -> Result<(), ()> {
        let condition_expr = &condition_expr.expr;
        self.emit_condition(condition_expr);

        Ok(())
    }

    fn branch_start_true_path(&mut self, id: NodeIndex) -> Result<(), ()> {
        if let Node::BranchMerge(_) = *self.cfg.node_weight(id) {
            self.output.emit_line("{ /* EMPTY */}");
        }

        Ok(())
    }

    fn branch_end_true_path(&mut self, _id: NodeIndex, _: &BranchingData) -> Result<(), ()> {
        // Emit nothing
        Ok(())
    }

    fn branch_start_false_path(&mut self, id: NodeIndex) -> Result<(), ()> {
        if let Node::BranchMerge(_) = *self.cfg.node_weight(id) {
            self.output.emit_line("else { /* EMPTY */}");
            self.output.line_pad();
        } else {
            self.output.emit_fmt(" else ");
        }

        Ok(())
    }

    fn branch_end_false_path(&mut self, _id: NodeIndex, _: &BranchingData) -> Result<(), ()> {
        // Emit nothing
        Ok(())
    }
}

#[allow(non_snake_case)]
mod RustGenFmt {
    use crate::ast::{BinOp, UniOp};
    use crate::analysis::*;

    pub fn field_id(id: FieldId) -> String {
        format!("_field{}", id.raw())
    }

    pub fn tmp_id(id: TmpId) -> String {
        format!("_tmp{}", id.raw())
    }

    pub fn var_id(id: VarId) -> String {
        format!("_var{}", id.raw())
    }

    pub fn fn_id(id: FnId) -> String {
        format!("_fn{}", id.raw())
    }

    pub fn type_id(id: TypeId) -> String {
        format!("_type{}", id.raw())
    }

    pub fn mod_id(id: ModuleId) -> String {
        format!("_mod{}", id.raw())
    }

    pub fn new_value(str: String) -> String {
        format!("Rc::new(RefCell::new({}))", str)
    }

    pub fn clone_value(str: String) -> String {
        format!("({}).clone()", str)
    }

    pub fn borrow_ref_mut(str: String) -> String {
        format!("({}).borrow_mut()", str)
    }

    pub fn borrow_ref(str: String) -> String {
        format!("({}).borrow()", str)
    }

    pub fn borrow_mut(str: String) -> String {
        format!("(*(({}).borrow_mut()))", str)
    }

    pub fn rustify_type(str: String) -> String {
        format!("Rc<RefCell<{}>>", str)
    }

    pub fn borrow(str: String) -> String {
        format!("(*(({}).borrow()))", str)
    }

    pub fn uni_op(op: &UniOp) -> String {
        use crate::ast::UniOp::*;

        match *op {
            Ref => unimplemented!(),
            Deref => unimplemented!(),
            Negate => "-".to_string(),
            LogicalInvert => "!".to_string(),
        }
    }

    pub fn bin_op(op: &BinOp) -> String {
        use crate::ast::BinOp::*;
        match *op {
            Add => "+".to_string(),
            Sub => "-".to_string(),
            Mul => "*".to_string(),
            Div => "/".to_string(),
            Mod => "%".to_string(),

            LogicalAnd => "&&".to_string(),
            LogicalOr => "||".to_string(),
            GreaterEq => ">=".to_string(),
            LesserEq => "<=".to_string(),
            Greater => ">".to_string(),
            Lesser => "<".to_string(),
            Eq => "==".to_string(),
            InEq => "!=".to_string(),
        }
    }
}
