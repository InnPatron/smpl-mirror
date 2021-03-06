use crate::span::Span;

use super::semantic_data::{BranchingId, LoopId};
use super::typed_ast;

#[derive(Clone, Debug)]
pub struct BasicBlock {
    graph: Vec<BlockNode>,
}

impl BasicBlock {
    pub fn new() -> BasicBlock {
        BasicBlock { graph: Vec::new() }
    }

    pub fn append(&mut self, node: BlockNode) {
        self.graph.push(node);
    }

    pub fn start(&self) -> usize {
        0
    }

    pub fn end(&self) -> usize {
        self.graph.len() - 1
    }

    pub fn graph(&self) -> &[BlockNode] {
        &self.graph
    }

    pub fn graph_mut(&mut self) -> &mut [BlockNode] {
        &mut self.graph
    }

    pub fn len(&self) -> usize {
        self.graph.len()
    }

    pub fn is_empty(&self) -> bool {
        self.graph.len() == 0
    }
}

#[derive(Clone, Debug)]
pub enum BlockNode {
    Expr(ExprData),

    Assignment(AssignmentData),
    LocalVarDecl(LocalVarDeclData),
}

#[derive(Clone, Debug)]
pub enum Node {
    Start,
    End,

    EnterScope,
    ExitScope,

    Return(ReturnData),
    Break(LoopData),
    Continue(LoopData),

    BranchSplit(BranchingData, ExprData),
    BranchMerge(BranchingData),

    LoopHead(LoopData, ExprData),
    LoopFoot(LoopData),

    Block(BasicBlock),
}

#[derive(Clone, Debug)]
pub enum Edge {
    Normal,
    True,
    False,
    BackEdge,
}

#[derive(Debug, Clone)]
pub struct BranchingData {
    pub branch_id: BranchingId,
}

#[derive(Debug, Clone)]
pub struct ExprData {
    pub expr: typed_ast::Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct AssignmentData {
    pub assignment: typed_ast::Assignment,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct LocalVarDeclData {
    pub decl: typed_ast::LocalVarDecl,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct LoopData {
    pub loop_id: LoopId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ReturnData {
    pub expr: Option<typed_ast::Expr>,
    pub span: Span,
}
