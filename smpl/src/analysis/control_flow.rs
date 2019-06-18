use petgraph::graph;
use petgraph::visit::EdgeRef;
use petgraph::Direction;

use crate::ast;

use crate::span::Span;

use super::error::{AnalysisError, ControlFlowError};
use super::expr_flow;
use super::semantic_data::{LoopId, ScopedData, Universe};
use super::type_cons::*;
use super::type_resolver::resolve_types;
use super::typed_ast;

use super::control_data::*;

macro_rules! node_w {
    ($CFG: expr, $node: expr) => {
        $CFG.graph().node_weight($node).unwrap()
    };
}

macro_rules! neighbors {
    ($CFG: expr, $node: expr) => {
        $CFG.graph().neighbors_directed($node, Direction::Outgoing)
    };
}

macro_rules! append_node {
    ($CFG: expr, $head: expr, $previous: expr, $to_insert: expr) => {
        append_node!($CFG, $head, $previous, $to_insert, Edge::Normal)
    };

    ($CFG: expr, $head: expr, $previous: expr, $to_insert: expr, $edge: expr) => {
        let temp = $CFG.graph.add_node($to_insert);
        if let Some(previous_node) = $previous {
            $CFG.graph.add_edge(previous_node, temp, $edge);
        }

        if $head.is_none() {
            $head = Some(temp);
        }

        $previous = Some(temp);
    };
}

macro_rules! append_node_index {
    ($CFG: expr, $head: expr, $previous: expr, $to_insert: expr) => {
        append_node_index!($CFG, $head, $previous, $to_insert, Edge::Normal)
    };

    ($CFG: expr, $head: expr, $previous: expr, $to_insert: expr, $edge: expr) => {
        if let Some(previous_node) = $previous {
            $CFG.graph.add_edge(previous_node, $to_insert, $edge);
        }

        if $head.is_none() {
            $head = Some($to_insert);
        }

        $previous = Some($to_insert);
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BranchData {
    head: Option<graph::NodeIndex>,
    foot: Option<graph::NodeIndex>,
}

#[derive(Clone, Debug)]
pub struct CFG {
    graph: graph::Graph<Node, Edge>,
    start: graph::NodeIndex,
    end: graph::NodeIndex,
}

impl CFG {
    pub fn graph(&self) -> &graph::Graph<Node, Edge> {
        &self.graph
    }

    pub fn node_weight(&self, node: graph::NodeIndex) -> &Node {
        self.graph.node_weight(node).unwrap()
    }

    pub fn neighbors_out(&self, node: graph::NodeIndex) -> graph::Neighbors<Edge> {
        self.graph.neighbors_directed(node, Direction::Outgoing)
    }

    pub fn neighbors_in(&self, node: graph::NodeIndex) -> graph::Neighbors<Edge> {
        self.graph.neighbors_directed(node, Direction::Incoming)
    }

    #[allow(unused_assignments)]
    ///
    /// Generate the control flow graph.
    /// Only performs continue/break statement checking (necessary for CFG generation).
    ///
    pub fn generate(
        universe: &Universe,
        body: ast::AstNode<ast::Block>,
        fn_type: &TypeCons,
        fn_scope: &ScopedData,
    ) -> Result<Self, AnalysisError> {
        let mut cfg = {
            let mut graph = graph::Graph::new();
            let start = graph.add_node(Node::Start);
            let end = graph.add_node(Node::End);

            CFG {
                graph: graph,
                start: start,
                end: end,
            }
        };

        // Start with Node::Start

        let (body, _) = body.to_data();
        let instructions = body.0;
        cfg.generate_scoped_block(universe, instructions.iter())?;

        Ok(cfg)
    }

    fn generate_scoped_block<'a, 'b, T>(&'a mut self, universe: &'b Universe, mut instructions: T) 
        -> Result<BranchData, ControlFlowError> 
        where T: Iterator<Item=&'a ast::Stmt> {
        use crate::ast::*;

        let mut previous: Option<graph::NodeIndex> = None;
        let mut head: Option<graph::NodeIndex> = None;

        let mut current_block = BasicBlock::new();
        let mut loop_data: Option<(graph::NodeIndex, graph::NodeIndex, LoopId)> = None;

        current_block.append(BlockNode::EnterScope);

        while let Some(next) = instructions.next() {
            match next {
                Stmt::Expr(expr) => {
                    let (ast_expr, span) = expr.to_data();
                    let expr = expr_flow::flatten(universe, ast_expr);
                    current_block.append(BlockNode::Expr(ExprData {
                        expr: expr,
                        span: span,
                    }));
                }

                Stmt::ExprStmt(expr_stmt) => {
                    let (expr_stmt, expr_stmt_span) = expr_stmt.to_data();
                    match expr_stmt {

                        // Append assignment node to current basic block
                        ExprStmt::Assignment(assignment) => {
                            let assignment = typed_ast::Assignment::new(universe, assignment);
                            current_block.append(BlockNode::Assignment(AssignmentData {
                                assignment: assignment,
                                span: expr_stmt_span,
                            }));
                        },

                        // Append local variable declaration node to current basic block
                        ExprStmt::LocalVarDecl(decl) => {
                            let decl = typed_ast::LocalVarDecl::new(universe, decl, expr_stmt_span);
                            current_block.append(BlockNode::LocalVarDecl(LocalVarDeclData {
                                decl: decl,
                                span: expr_stmt_span,
                            }));
                        },

                        // Append return node to current basic block
                        ExprStmt::Return(span, expr) => {
                            let expr = expr.map(|expr| expr_flow::flatten(universe, expr));
                            current_block.append(BlockNode::Return(ReturnData {
                                expr: expr,
                                span: span,
                            }));
                        },

                        // Append break node to current basic block
                        ExprStmt::Break(span) => {
                            match loop_data {
                                Some((_, foot, loop_id)) => {
                                    current_block.append(BlockNode::Break(LoopData {
                                        loop_id: loop_id,
                                        span: span,
                                    }));
                                },

                                None => return Err(ControlFlowError::BadBreak(span)), 
                            }
                        },

                        // Append continue node to current basic block
                        ExprStmt::Continue(span) => {
                            match loop_data {
                                Some((loop_head, _, loop_id)) => {
                                    current_block.append(BlockNode::Continue(LoopData {
                                        loop_id: loop_id,
                                        span: span,
                                    }));
                                },

                                None => return Err(ControlFlowError::BadContinue(span)),
                            }
                        },

                        _ => unimplemented!()
                    }
                }
            }

        }


        current_block.append(BlockNode::ExitScope);
        append_node!(self, previous, head, Node::Block(current_block));
        
        Ok(BranchData {
            head: head,
            foot: previous,
        })
    }
}

#[cfg(test)]
#[cfg_attr(rustfmt, rustfmt_skip)]
mod tests {
    use super::*;
    use crate::parser::*;
    use crate::parser::parser::*;
    use petgraph::dot::{Config, Dot};
    use petgraph::Direction;

    use super::super::semantic_data::{TypeId, Universe};

    macro_rules! edges {
        ($CFG: expr, $node: expr) => {
            $CFG.graph.edges_directed($node, Direction::Outgoing)
        }
    }

    macro_rules! node_w {
        ($CFG: expr, $node: expr) => {
            $CFG.graph.node_weight($node).unwrap()
        }
    }

    fn expected_app(tc: TypeId) -> AbstractType {
        AbstractType::App {
            type_cons: tc,
            args: None
        }
    }

    fn fn_type_cons(params: Vec<AbstractType>, return_type: AbstractType) -> TypeCons {
        let tc = TypeCons::Function {
            parameters: params,
            return_type: return_type,
            type_params: TypeParams::empty(),
        };

        tc
    }

    #[test]
    fn linear_cfg_generation() {
        let input = "fn test(arg: int) {
let a: int = 2;
let b: int = 3;
}";
        let mut input = buffer_input(input);
        let universe = Universe::std();
        let fn_type = fn_type_cons(vec![expected_app(universe.int())], expected_app(universe.unit()));
        let fn_def = testfn_decl(&mut input).unwrap();
        let scope = universe.std_scope();
        let cfg = CFG::generate(&universe, fn_def.body.clone(), &fn_type, &scope).unwrap();

        println!("{:?}", Dot::with_config(&cfg.graph, &[Config::EdgeNoLabel]));

        {
            irmatch!(*cfg.graph.node_weight(cfg.start).unwrap(); Node::Start => ());
            irmatch!(*cfg.graph.node_weight(cfg.end).unwrap(); Node::End => ());
            // start -> enter_scope -> var decl -> var decl -> implicit return -> exit_scope -> end
            assert_eq!(cfg.graph.node_count(), 7);

            let mut start_neighbors = neighbors!(cfg, cfg.start);

            let enter = start_neighbors.next().unwrap();
            let mut enter_neighbors = neighbors!(cfg, enter);
            match *node_w!(cfg, enter) {
                Node::EnterScope => (),
                ref n @ _ => panic!("Expected to find Node::EnterScope. Found {:?}", n),
            }

            let var_decl_1 = enter_neighbors.next().unwrap();
            let mut var_decl_1_neighbors = neighbors!(cfg, var_decl_1);
            match *node_w!(cfg, var_decl_1) {
                Node::LocalVarDecl(_) => (),
                ref n @ _ => panic!("Expected to find Node::LocalVarDecl. Found {:?}", n),
            }

            let var_decl_2 = var_decl_1_neighbors.next().unwrap();
            let mut var_decl_2_neighbors = neighbors!(cfg, var_decl_2);
            match *node_w!(cfg, var_decl_2) {
                Node::LocalVarDecl(_) => (),
                ref n @ _ => panic!("Expected to find Node::LocalVarDecl. Found {:?}", n),
            }

            let ret = var_decl_2_neighbors.next().unwrap();
            let mut ret_neighbors = neighbors!(cfg, ret);
            match *node_w!(cfg, ret) {
                Node::Return(..) => (),
                ref n @ _ => panic!("Expected to find Node::Return. Found {:?}", n),
            }

            let exit = ret_neighbors.next().unwrap();
            let mut exit_neighbors = neighbors!(cfg, exit);
            match *node_w!(cfg, exit) {
                Node::ExitScope => (),
                ref n @ _ => panic!("Expected to find Node::ExitScope. Found {:?}", n),
            }

            let end = exit_neighbors.next().unwrap();
            let end_neighbors = neighbors!(cfg, end);
            assert_eq!(end_neighbors.count(), 0);
            match *node_w!(cfg, end) {
                Node::End => (),
                ref n @ _ => panic!("Expected to find Node::End. Found {:?}", n),
            }
        }
    }

    #[test]
    fn branching_cfg_generation() {
        let input = "fn test(arg: int) {
if (test) {
    let c: int = 4;
}
}";
        let mut input = buffer_input(input);

        let universe = Universe::std();
        let fn_type = fn_type_cons(vec![expected_app(universe.int())], expected_app(universe.unit()));
        let fn_def = testfn_decl(&mut input).unwrap();
        let scope = universe.std_scope();
        let cfg = CFG::generate(&universe, fn_def.body.clone(), &fn_type, &scope).unwrap();

        println!("{:?}", Dot::with_config(&cfg.graph, &[Config::EdgeNoLabel]));

        {
            irmatch!(*cfg.graph.node_weight(cfg.start).unwrap(); Node::Start => ());
            irmatch!(*cfg.graph.node_weight(cfg.end).unwrap(); Node::End => ());

            // start -> enter_scope -> branch_split -> condition
            //      -[true]> {
            //          -> enter_scope
            //          -> var decl
            //          -> exit_scope
            //      } ->        >>___ branch_merge ->
            //        -[false]> >>
            //      implicit return -> exit_scope -> end
            assert_eq!(cfg.graph.node_count(), 11);

            let mut start_neighbors = neighbors!(cfg, cfg.start);

            let enter = start_neighbors.next().unwrap();
            let mut enter_neighbors = neighbors!(cfg, enter);
            match *node_w!(cfg, enter) {
                Node::EnterScope => (),
                ref n @ _ => panic!("Expected to find Node::EnterScope. Found {:?}", n),
            }

            let mut merge = None;

            let branch_split = enter_neighbors.next().expect("Looking for BranchSplit");
            let mut branch_split_neighbors = neighbors!(cfg, branch_split);

            match *node_w!(cfg, branch_split) {
                Node::BranchSplit(_) => (),
                ref n @ _ => panic!("Expected BranchSplit node. Found {:?}", n),
            }

            // Check condition node
            let condition = branch_split_neighbors.next().expect("Looking for condition node");
            let condition_node = cfg.graph.node_weight(condition).unwrap();
            {
                if let Node::Condition(_) = *condition_node {
                    let mut edges = cfg.graph.edges_directed(condition, Direction::Outgoing);
                    assert_eq!(edges.clone().count(), 2);

                    let mut found_true_edge = false;
                    let mut found_false_edge = false;

                    // Look for True False edges and verify
                    for edge in edges {
                        if let Edge::True = *edge.weight() {
                            let target = edge.target();

                            match *cfg.graph.node_weight(target).unwrap() {
                                Node::EnterScope => {
                                    let mut neighbors =
                                        cfg.graph.neighbors_directed(target, Direction::Outgoing);
                                    let decl = neighbors.next().unwrap();

                                    match *cfg.graph.node_weight(decl).unwrap() {
                                        Node::LocalVarDecl(_) => (),
                                        ref n @ _ => panic!(
                                            "Expected to find Node::LocalVarDecl. Found {:?}",
                                            n
                                        ),
                                    }

                                    let mut neighbors =
                                        cfg.graph.neighbors_directed(decl, Direction::Outgoing);
                                    let exit_scope = neighbors.next().unwrap();

                                    match *cfg.graph.node_weight(exit_scope).unwrap() {
                                        Node::ExitScope => (),

                                        ref n @ _ => panic!(
                                            "Expected to find Node::ExitScope. Found {:?}",
                                            n
                                        ),
                                    }
                                }

                                ref n @ _ => {
                                    panic!("Expected to find Node::EnterScope. Found {:?}", n)
                                }
                            }

                            found_true_edge = true;
                        } else if let Edge::False = *edge.weight() {
                            let target = edge.target();

                            if let Node::BranchMerge(_) = *cfg.graph.node_weight(target).unwrap() {
                                merge = Some(target);
                            }

                            found_false_edge = true;
                        }
                    }

                    assert!(found_true_edge);
                    assert!(found_false_edge);
                } else {
                    panic!("Not a condition node");
                }
            }

            let merge = merge.unwrap();

            let return_n = cfg.graph.neighbors(merge).next().unwrap();
            let mut return_neighbors = neighbors!(cfg, return_n);
            match *node_w!(cfg, return_n) {
                Node::Return(..) => (),
                ref n @ _ => panic!("Expected to find Node::Return. Found {:?}", n),
            }

            let exit = return_neighbors.next().unwrap();
            let mut exit_neighbors = neighbors!(cfg, exit);
            match *node_w!(cfg, exit) {
                Node::ExitScope => (),
                ref n @ _ => panic!("Expected to find Node::ExitScope. Found {:?}", n),
            }

            let end = exit_neighbors.next().unwrap();
            let end_neighbors = neighbors!(cfg, end);
            match *node_w!(cfg, end) {
                Node::End => {
                    assert_eq!(end_neighbors.count(), 0);
                }
                ref n @ _ => panic!("Expected to find Node::ExitScope. Found {:?}", n),
            }
        }
    }

    #[test]
    fn complex_branching_cfg_generation() {
        let input = "fn test(arg: int) {
    if (false) {
        let c: int = 4;
    } elif (true) {

    } else {

    }
}";
        let mut input = buffer_input(input);
        let universe = Universe::std();
        let fn_type = fn_type_cons(vec![expected_app(universe.int())], expected_app(universe.unit()));
        
        let fn_def = testfn_decl(&mut input).unwrap();
        let scope = universe.std_scope();
        let cfg = CFG::generate(&universe, fn_def.body.clone(), &fn_type, &scope).unwrap();

        println!("{:?}", Dot::with_config(&cfg.graph, &[Config::EdgeNoLabel]));

        {
            // start -> enter_scope -> branch_split -> condition(B)
            //      -[true]> {
            //          enter_scope ->
            //          local_var_decl ->
            //          exit_scope ->
            //      } -> branch_merge(A)
            //
            //      -[false]> condition(C)
            //           -[true]> branch_merge(A)
            //
            //           -[false]> branch_merge(A)
            //
            // branch_merge(A) -> implicit_return -> exit_scope -> end
            //

            assert_eq!(cfg.graph.node_count(), 12);

            let mut start_neighbors = neighbors!(cfg, cfg.start);
            assert_eq!(start_neighbors.clone().count(), 1);

            let enter = start_neighbors.next().unwrap();
            let mut enter_neighbors = neighbors!(cfg, enter);
            match *node_w!(cfg, enter) {
                Node::EnterScope => (),
                ref n @ _ => panic!("Expected to find Node::Enter. Found {:?}", n),
            }

            let branch_split = enter_neighbors.next().unwrap();
            let mut branch_split_neighbors = neighbors!(cfg, branch_split);
            match *node_w!(cfg, branch_split) {
                Node::BranchSplit(_) => (), // Success

                ref n @ _ => panic!("Expected a condition node. Found {:?}", n),
            }

            let condition_b = branch_split_neighbors.next().unwrap();
            match *node_w!(cfg, condition_b) {
                Node::Condition(_) => (), // Success

                ref n @ _ => panic!("Expected a condition node. Found {:?}", n),
            }

            let condition_b_edges = edges!(cfg, condition_b);
            let mut condition_c = None;
            let mut condition_b_true = None;

            assert_eq!(condition_b_edges.clone().count(), 2);
            for edge in condition_b_edges {
                match *edge.weight() {
                    Edge::True => condition_b_true = Some(edge.target()),
                    Edge::False => condition_c = Some(edge.target()),

                    ref e @ _ => panic!("Expected true or false edge. Found {:?}", e),
                }
            }

            // condition b TRUE branch

            let enter =
                condition_b_true.expect("Missing true edge connecting to variable declaration");
            let mut enter_neighbors = neighbors!(cfg, enter);
            match *node_w!(cfg, enter) {
                Node::EnterScope => (),
                ref n @ _ => panic!("Expected Node::EnterScope. Found {:?}", n),
            }

            let var_decl = enter_neighbors.next().unwrap();
            let mut var_decl_neighbors = neighbors!(cfg, var_decl);
            assert_eq!(var_decl_neighbors.clone().count(), 1);
            match *node_w!(cfg, var_decl) {
                Node::LocalVarDecl(_) => (),

                ref n @ _ => panic!("Expected local variable declartion. Found {:?}", n),
            }

            let exit = var_decl_neighbors.next().unwrap();
            let mut exit_neighbors = neighbors!(cfg, exit);
            match *node_w!(cfg, exit) {
                Node::ExitScope => (),
                ref n @ _ => panic!("Expected Node::ExitScope. Found {:?}", n),
            }

            let merge = exit_neighbors.next().unwrap();
            match *node_w!(cfg, merge) {
                Node::BranchMerge(_) => (),

                ref n @ _ => panic!("Expected Node::BranchMerge. Found {:?}", n),
            }

            // condition b FALSE branch (condition c)
            let condition_c = condition_c.expect("Missing false edge connecting to Condition C");
            let condition_c_edges = edges!(cfg, condition_c);
            let mut truth_target = None;
            let mut false_target = None;

            assert_eq!(condition_c_edges.clone().count(), 2);
            for edge in condition_c_edges {
                match *edge.weight() {
                    Edge::True => truth_target = Some(edge.target()),
                    Edge::False => false_target = Some(edge.target()),

                    ref e @ _ => panic!("Expected true or false edge. Found {:?}", e),
                }
            }

            let truth_target = truth_target.unwrap();
            let false_target = false_target.unwrap();

            assert_eq!(truth_target, false_target);
            match *node_w!(cfg, truth_target) {
                Node::BranchMerge(_) => (),
                ref n @ _ => panic!("Expected BranchMerge. Found {:?}", n),
            }

            match *node_w!(cfg, false_target) {
                Node::BranchMerge(_) => (),
                ref n @ _ => panic!("Expected BranchMerge. Found {:?}", n),
            }

            let branch_merge = truth_target;
            let mut branch_merge_neighbors = neighbors!(cfg, branch_merge);
            assert_eq!(branch_merge_neighbors.clone().count(), 1);

            let implicit_return = branch_merge_neighbors.next().unwrap();
            let mut implicit_return_neighbors = neighbors!(cfg, implicit_return);
            assert_eq!(implicit_return_neighbors.clone().count(), 1);
            match *node_w!(cfg, implicit_return) {
                Node::Return(..) => (),
                ref n @ _ => println!("Expected return node. Found {:?}", n),
            }

            let exit = implicit_return_neighbors.next().unwrap();
            let mut exit_neighbors = neighbors!(cfg, exit);
            match *node_w!(cfg, exit) {
                Node::ExitScope => (),
                ref n @ _ => panic!("Expected to find Node::Exit. Found {:?}", n),
            }

            let end = exit_neighbors.next().unwrap();
            irmatch!(*node_w!(cfg, end); Node::End => ());
        }
    }

    #[test]
    fn while_loop_generation() {
        let input = "fn test(arg: int) {
    while (true) {
        
    }
}";
        let mut input = buffer_input(input);
        let universe = Universe::std();
        let fn_type = fn_type_cons(vec![expected_app(universe.int())], expected_app(universe.unit()));
        
        let fn_def = testfn_decl(&mut input).unwrap();
        let scope = universe.std_scope();
        let cfg = CFG::generate(&universe, fn_def.body.clone(), &fn_type, &scope).unwrap();

        println!("{:?}", Dot::with_config(&cfg.graph, &[Config::EdgeNoLabel]));

        // start -> enter_scope -> loop_head(A) -> condition(B)
        //       -[true]> enter_scope exit_scope loop_foot(A)
        //       -[false]> loop_foot(A)
        // loop_foot(A) -> implicit_return -> exit_scope -> end
        // loop_head(A) << loop_foot(A)
        //

        assert_eq!(cfg.graph.node_count(), 10);

        let mut start_neighbors = neighbors!(cfg, cfg.start);
        assert_eq!(start_neighbors.clone().count(), 1);

        let enter = start_neighbors.next().unwrap();
        let mut enter_neighbors = neighbors!(cfg, enter);
        match *node_w!(cfg, enter) {
            Node::EnterScope => (),
            ref n @ _ => panic!("Expected to find Node::Enter. Found {:?}", n),
        }

        let loop_id;
        let loop_head = enter_neighbors.next().unwrap();
        match *node_w!(cfg, loop_head) {
            Node::LoopHead(ref loop_data) => loop_id = loop_data.loop_id,
            ref n @ _ => panic!("Expected to find Node::LoopHead. Found {:?}", n),
        }

        let mut head_neighbors = neighbors!(cfg, loop_head);
        assert_eq!(head_neighbors.clone().count(), 1);

        let condition = head_neighbors.next().unwrap();
        match *node_w!(cfg, condition) {
            Node::Condition(_) => (),
            ref n @ _ => panic!("Expected condition node. Found {:?}", n),
        }

        let condition_edges = edges!(cfg, condition);
        assert_eq!(condition_edges.clone().count(), 2);

        let mut truth_target = None;
        let mut false_target = None;
        for edge in condition_edges {
            match *edge.weight() {
                Edge::True => truth_target = Some(edge.target()),
                Edge::False => false_target = Some(edge.target()),

                ref e @ _ => panic!("Expected true or false edge. Found {:?}", e),
            }
        }

        let truth_target = truth_target.unwrap();
        let false_target = false_target.unwrap();
        match *node_w!(cfg, truth_target) {
            Node::EnterScope => (),
            ref n @ _ => panic!("Expected to find Node::EnterScope. Found {:?}", n),
        }

        let mut enter_neighbors = neighbors!(cfg, truth_target);
        let exit = enter_neighbors.next().unwrap();
        let mut exit_neighbors = neighbors!(cfg, exit);
        match *node_w!(cfg, exit) {
            Node::ExitScope => (),
            ref n @ _ => panic!("Expected to find Node::ExitScope. Found {:?}", n),
        }

        let foot = exit_neighbors.next().unwrap();
        let mut foot_neighbors = neighbors!(cfg, foot);
        match *node_w!(cfg, foot) {
            Node::LoopFoot(ref loop_data) => assert_eq!(loop_data.loop_id, loop_id),
            ref n @ _ => panic!("Expected to find Node::LoopFoot. Found {:?}", n),
        }

        assert_eq!(foot, false_target);

        assert_eq!(foot_neighbors.clone().count(), 2);

        let implicit_return = foot_neighbors.next().unwrap();
        let mut return_neighbors = neighbors!(cfg, implicit_return);
        assert_eq!(return_neighbors.clone().count(), 1);
        match *node_w!(cfg, implicit_return) {
            Node::Return(..) => (),
            ref n @ _ => panic!("Expected return node. Found {:?}", n),
        }

        let exit = return_neighbors.next().unwrap();
        let mut exit_neighbors = neighbors!(cfg, exit);
        match *node_w!(cfg, exit) {
            Node::ExitScope => (),
            ref n @ _ => panic!("Expected to find Node::ExitScope. Found {:?}", n),
        }

        let end = exit_neighbors.next().unwrap();
        let end_neighbors = neighbors!(cfg, end);
        assert_eq!(end_neighbors.count(), 0);
        match *node_w!(cfg, end) {
            Node::End => (),
            ref n @ _ => panic!("Expected to find Node::End. Found {:?}", n),
        }
    }
}
