use semantic_ck::{TypeId, FnId};
use ast::{Path, Ident, BinOp, UniOp};

#[derive(Clone, Debug)]
pub enum Err {
    ControlFlowErr(ControlFlowErr),
    TypeErr(TypeErr),
    MultipleMainFns,
    UnknownType(Path),
    UnknownVar(Ident),
    UnknownFn(Path),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControlFlowErr {
    MissingReturn,
    BadBreak,
    BadContinue,
}

impl From<ControlFlowErr> for Err {
    fn from(err: ControlFlowErr) -> Err {
        Err::ControlFlowErr(err)
    }
}

#[derive(Clone, Debug)]
pub enum TypeErr {
    LhsRhsInEq(TypeId, TypeId),
    InEqFnReturn {
        expr: TypeId,
        fn_return: TypeId,
    },

    UnexpectedType {
        found: TypeId,
        expected: TypeId,
    },

    Arity {
        fn_type: TypeId,
        found_args: usize,
        expected_param: usize,
    },

    BinOp {
        op: BinOp,
        expected: Vec<TypeId>,
        lhs: TypeId,
        rhs: TypeId,
    },

    UniOp {
        op: UniOp,
        expected: Vec<TypeId>,
        expr: TypeId,
    },

    ArgMismatch {
        fn_id: FnId,
        index: usize,
        arg: TypeId,
        param: TypeId,
    },

    NotAStruct {
        path: Path,
        index: usize,
        invalid_type: TypeId,
        root_type: TypeId,
    },

    UnknownField {
        name: Ident,
        struct_type: TypeId,
    },
}

impl From<TypeErr> for Err {
    fn from(err: TypeErr) -> Err {
        Err::TypeErr(err)
    }
}
