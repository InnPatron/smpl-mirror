use semantic_ck::{FnId, TypeId};
use ast::{BinOp, Ident, Path, UniOp};

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

    FieldAccessOnNonStruct {
        path: Path,
        index: usize,
        invalid_type: TypeId,
        root_type: TypeId,
    },

    NotAStruct {
        type_name: Path,
        found: TypeId,
    },

    StructNotFullyInitialized {
        type_name: Path,
        struct_type: TypeId,
        missing_fields: Vec<Ident>,
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