use std::collections::HashMap;
use std::fmt;

use crate::span::Span;
use crate::ast_node::AstNode;
use crate::expr_ast::{ Expr, Block };

pub struct Module {
    pub ident: AstNode<Ident>,
    pub top_levels: Vec<DeclStmt>,
}

#[derive(Clone)]
pub enum DeclStmt {
    Use(AstNode<UseDecl>),
    Opaque(AstNode<Opaque>),
    Struct(AstNode<Struct>),
    Function(AstNode<Function>),
    BuiltinFunction(AstNode<BuiltinFunction>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct UseDecl(pub AstNode<Ident>);

#[derive(Debug, Clone, PartialEq)]
pub struct BuiltinFunction {
    pub name: AstNode<Ident>,
    pub params: BuiltinFnParams,
    pub return_type: Option<AstNode<TypeAnnotation>>,
    pub annotations: Vec<Annotation>,
    pub type_params: Option<TypeParams>,
    pub where_clause: Option<WhereClause>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BuiltinFnParams {
    Unchecked,
    Checked(Option<Vec<AstNode<FnParameter>>>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: AstNode<Ident>,
    pub params: Option<Vec<AstNode<FnParameter>>>,
    pub return_type: Option<AstNode<TypeAnnotation>>,
    pub body: AstNode<Block>,
    pub annotations: Vec<Annotation>,
    pub type_params: Option<TypeParams>,
    pub where_clause: Option<WhereClause>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FnParameter {
    pub name: AstNode<Ident>,
    pub param_type: AstNode<TypeAnnotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Opaque {
    pub name: AstNode<Ident>,
    pub annotations: Vec<Annotation>,
    pub type_params: Option<TypeParams>,
    pub where_clause: Option<WhereClause>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Struct {
    pub name: AstNode<Ident>,
    pub body: StructBody,
    pub annotations: Vec<Annotation>,
    pub type_params: Option<TypeParams>,
    pub where_clause: Option<WhereClause>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhereClause(pub HashMap<AstNode<Ident>, Vec<AstNode<TypeAnnotation>>>);

#[derive(Debug, Clone, PartialEq)]
pub struct StructBody(pub Option<Vec<StructField>>);

#[derive(Debug, Clone, PartialEq)]
pub struct StructField {
    pub name: AstNode<Ident>,
    pub field_type: AstNode<TypeAnnotation>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TypeAnnotation {
    Path(TypedPath),
    Array(Box<AstNode<TypeAnnotation>>, u64),
    FnType(
        Option<TypeParams>,
        Option<Vec<AstNode<TypeAnnotation>>>,
        Option<Box<AstNode<TypeAnnotation>>>,
    ),
    WidthConstraint(Vec<AstNode<WidthConstraint>>),
}

// TODO: May need to manually implement PartialEq
// Shouldn't matter b/c this is purely for syntactic comparison
#[derive(Debug, Clone, PartialEq)]
pub struct TypeParams {
    pub params: Vec<AstNode<Ident>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WidthConstraint {
    BaseStruct(AstNode<TypeAnnotation>),
    Anonymous(Vec<(AstNode<Ident>, AstNode<TypeAnnotation>)>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypedPath {
    pub base: ModulePath,
    pub params: Vec<AstNode<TypeAnnotation>>,
}

impl TypedPath {
    pub fn nil_arity(base: ModulePath) -> Self {
        TypedPath {
            base,
            params: Vec::with_capacity(0),
        }
    }

    pub fn n_arity(base: ModulePath, params: Vec<AstNode<TypeAnnotation>>) -> Self {
        TypedPath {
            base,
            params,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModulePath(pub Vec<AstNode<Ident>>);

#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub keys: Vec<(Ident, Option<String>)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Ident(pub String);

impl Ident {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl<T> From<T> for Ident where T: Into<String> {
    fn from(s: T) -> Ident {
        Ident(s.into())
    }
}

impl fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
