use std::str::FromStr;
use std::fmt;
use std::slice::Iter;

use ascii::AsciiString;

pub struct Program(pub Vec<DeclStmt>);

pub enum DeclStmt {
    Struct(Struct),
    Function(Function),
}

impl From<Struct> for DeclStmt {
    fn from(s: Struct) -> DeclStmt {
        DeclStmt::Struct(s)
    }
}

impl From<Function> for DeclStmt {
    fn from(f: Function) -> DeclStmt {
        DeclStmt::Function(f)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: Ident,
    pub params: Option<Vec<FnParameter>>,
    pub return_type: Option<Path>,
    pub body: Block,
}

impl Function {
    pub fn new(name: Ident, params: Option<Vec<FnParameter>>, return_type: Option<Path>, body: Block) -> Function {
        Function {
            name: name,
            params: params,
            return_type: return_type,
            body: body,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FnParameter {
    pub name: Ident,
    pub param_type: Path,
}

impl FnParameter {
    pub fn new(name: Ident, param_type: Path) -> FnParameter {
        FnParameter {
            name: name,
            param_type: param_type,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Struct {
    pub name: Ident,
    pub body: StructBody,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructBody(pub Option<Vec<StructField>>);

#[derive(Debug, Clone, PartialEq)]
pub struct StructField {
    pub name: Ident,
    pub field_type: Path,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    ExprStmt(ExprStmt),
    Expr(Expr),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExprStmt {
    If(If),
    While(While),
    LocalVarDecl(LocalVarDecl),
    Assignment(Assignment),
    Return(Expr),
    Break,
    Continue,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Assignment {
    pub name: Path,
    pub value: Expr,
}

impl Assignment {
    pub fn new(name: Path, value: Expr) -> Assignment {
        Assignment {
            name: name,
            value: value,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LocalVarDecl {
    pub var_type: Path,
    pub var_name: Ident,
    pub var_init: Expr,
}

impl LocalVarDecl {
    pub fn new(var_type: Path, var_name: Ident, var_init: Expr) -> LocalVarDecl {
        LocalVarDecl {
            var_type: var_type,
            var_name: var_name,
            var_init: var_init,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct If {
    pub branches: Vec<Branch>,
    pub default_block: Option<Block>
}

#[derive(Clone, Debug, PartialEq)]
pub struct Branch {
    pub conditional: Expr,
    pub block: Block,
}

#[derive(Clone, Debug, PartialEq)]
pub struct While {
    pub conditional: Expr,
    pub block: Block,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Bin(BinExpr),
    Uni(UniExpr),
    Literal(Literal),
    Variable(Ident),
    FieldAccess(Path),
    FnCall(FnCall),
}

#[derive(Clone, Debug, PartialEq)]
pub struct FnCall {
    pub name: Ident,
    pub args: Option<Vec<Expr>>,
}

impl FnCall {
    pub fn new(name: Ident, args: Option<Vec<Expr>>) -> FnCall {
        FnCall {
            name: name,
            args: args, 
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BinExpr {
    pub op: BinOp,
    pub lhs: Box<Expr>,
    pub rhs: Box<Expr>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,

    LogicalAnd,
    LogicalOr,
    GreaterEq,
    LesserEq,
    Greater,
    Lesser,
    Eq,
    InEq,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UniExpr {
    pub op: UniOp,
    pub expr: Box<Expr>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UniOp {
    Ref,
    Deref,
    Negate,
    LogicalInvert,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Literal {
    String(AsciiString),
    Int(i64),
    Float(f64),
    Bool(bool),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block(pub Vec<Stmt>);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Ident(pub AsciiString);

impl Ident {
    pub fn new(str: &str) -> Ident {
        Ident(AsciiString::from_str(str).unwrap())
    }
}

impl fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Path(pub Vec<Ident>);

impl Path {
    pub fn iter(&self) -> Iter<Ident> {
        self.0.iter()
    }
}

impl From<Ident> for Path {
    fn from(ident: Ident) -> Path {
        Path(vec![ident])
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let buffer = self.0.iter().fold(AsciiString::new(), 
                                        |mut buffer, ref item| {
                                            buffer.push_str(&item.0); 
                                            buffer
                                        });
        write!(f, "{}", buffer)
    }
}
