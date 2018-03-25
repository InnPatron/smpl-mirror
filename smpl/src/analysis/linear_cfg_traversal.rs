use petgraph::graph::NodeIndex;
use petgraph::Direction;
use petgraph::visit::EdgeRef;

use super::control_flow::*;
use super::typed_ast::*;

pub trait Passenger<E> {
    fn start(&mut self, id: NodeIndex) -> Result<(), E>;
    fn end(&mut self, id: NodeIndex) -> Result<(), E>;
    fn loop_head(&mut self, id: NodeIndex) -> Result<(), E>;
    fn loop_foot(&mut self, id: NodeIndex) -> Result<(), E>;
    fn cont(&mut self, id: NodeIndex) -> Result<(), E>;
    fn br(&mut self, id: NodeIndex) -> Result<(), E>;
    fn enter_scope(&mut self, id: NodeIndex) -> Result<(), E>;
    fn exit_scope(&mut self, id: NodeIndex) -> Result<(), E>;
    fn local_var_decl(&mut self, id: NodeIndex, decl: &LocalVarDecl) -> Result<(), E>;
    fn assignment(&mut self, id: NodeIndex, assign: &Assignment) -> Result<(), E>;
    fn expr(&mut self, id: NodeIndex, expr: &Expr) -> Result<(), E>;
    fn ret(&mut self, id: NodeIndex, expr: Option<&Expr>) -> Result<(), E>;

    fn loop_condition(&mut self, id: NodeIndex, e: &Expr) -> Result<(), E>;
    fn loop_start_true_path(&mut self, id: NodeIndex) -> Result<(), E>;
    fn loop_end_true_path(&mut self, id: NodeIndex) -> Result<(), E>;

    fn branch_split(&mut self, id: NodeIndex) -> Result<(), E>;
    fn branch_merge(&mut self, id: NodeIndex) -> Result<(), E>;
    fn branch_condition(&mut self, id: NodeIndex, e: &Expr) -> Result<(), E>;
    fn branch_start_true_path(&mut self, id: NodeIndex) -> Result<(), E>;
    fn branch_start_false_path(&mut self, id: NodeIndex) -> Result<(), E>;
    fn branch_end_true_path(&mut self, id: NodeIndex) -> Result<(), E>;
    fn branch_end_false_path(&mut self, id: NodeIndex) -> Result<(), E>;
}

pub struct Traverser<'a, 'b, E: 'b> {
    graph: &'a CFG,
    passenger: &'b mut Passenger<E>,
    previous_is_loop_head: bool,
    node_count: usize,
}

impl<'a, 'b, E> Traverser<'a, 'b, E> {
    pub fn new(graph: &'a CFG, passenger: &'b mut Passenger<E>) -> Traverser<'a, 'b, E> {
        Traverser {
            graph: graph,
            passenger: passenger,
            previous_is_loop_head: false,
            node_count: graph.graph().node_count(),
        }
    }

    pub fn traverse(mut self) -> Result<(), E> {
        let mut current = Some(self.graph.start());

        // Traverser::visit_node should be called AT MAX the number of nodes in the graph
        for _ in 0..self.node_count {
            match current {
                Some(to_visit) => current = self.visit_node(to_visit)?,
                None => break,
            }
        }

        if current.is_some() {
            panic!("Graph traversal error. Node::End should have returned None. If Node::End was reached, this panic should not be triggered.")
        }

        Ok(())
    }

    fn visit_node(&mut self, current: NodeIndex) -> Result<Option<NodeIndex>, E> {
        match *self.graph.node_weight(current) {
            Node::End => {
                self.passenger.end(current)?;
                self.previous_is_loop_head = false;
                Ok(None)
            }

            Node::Start => {
                self.passenger.start(current)?;
                self.previous_is_loop_head = false;
                Ok(Some(self.graph.next(current)))
            }

            Node::BranchSplit(id) => {
                self.passenger.branch_split(current)?;
                self.previous_is_loop_head = false;
                Ok(Some(self.graph.next(current)))
            }

            Node::BranchMerge(id) => {
                self.passenger.branch_merge(current)?;
                self.previous_is_loop_head = false;
                Ok(Some(self.graph.next(current)))
            }

            Node::LoopHead(_) => {
                self.passenger.loop_head(current)?;
                self.previous_is_loop_head = true;
                Ok(Some(self.graph.next(current)))
            }

            Node::LoopFoot(_) => {
                self.passenger.loop_foot(current)?;
                self.previous_is_loop_head = false;
                Ok(Some(self.graph.after_loop_foot(current)))
            }

            Node::Continue(_) => {
                self.passenger.cont(current)?;
                self.previous_is_loop_head = false;
                Ok(Some(self.graph.after_continue(current)))
            }

            Node::Break(_) => {
                self.passenger.br(current)?;
                self.previous_is_loop_head = false;
                Ok(Some(self.graph.after_break(current)))
            }

            Node::EnterScope => {
                self.passenger.enter_scope(current)?;
                self.previous_is_loop_head = false;
                Ok(Some(self.graph.next(current)))
            }

            Node::ExitScope => {
                self.passenger.exit_scope(current)?;
                self.previous_is_loop_head = false;
                Ok(Some(self.graph.next(current)))
            }

            Node::LocalVarDecl(ref decl) => {
                self.passenger.local_var_decl(current, decl)?;
                self.previous_is_loop_head = false;
                Ok(Some(self.graph.next(current)))
            }

            Node::Assignment(ref assign) => {
                self.passenger.assignment(current, assign)?;
                self.previous_is_loop_head = false;
                Ok(Some(self.graph.next(current)))
            }

            Node::Expr(ref expr) => {
                self.passenger.expr(current, expr)?;
                self.previous_is_loop_head = false;
                Ok(Some(self.graph.next(current)))
            }

            Node::Return(ref ret_expr) => {
                self.passenger.ret(current, ret_expr.as_ref())?;
                self.previous_is_loop_head = false;
                Ok(Some(self.graph.after_return(current)))
            }

            Node::Condition(ref condition) => {
                if self.previous_is_loop_head {
                    // Loop condition
                    self.previous_is_loop_head = false;
                    self.passenger.loop_condition(current, condition)?;

                    let (true_path, false_path) = self.graph.after_condition(current);
                    self.passenger.loop_start_true_path(true_path)?;

                    let mut current_node = true_path;
                    let mut found_foot = false;
                    for _ in 0..self.node_count {
                        match *self.graph.node_weight(current_node) {
                            Node::LoopFoot(_) => {
                                self.passenger.loop_end_true_path(current_node)?;
                                found_foot = true;
                                break;
                            }

                            _ => (),
                        }

                        match self.visit_node(current_node)? {
                            Some(next) => current_node = next,
                            None => return Ok(None),
                        }
                    }

                    if found_foot == false {
                        panic!("Traversed the rest of the graph but did not find a Node::LoopFoot.");
                    }

                    match *self.graph.node_weight(false_path) {
                        Node::LoopFoot(_) => (),
                        ref n @ _ => println!("Loop condition should be connected to Node::LoopFoot along the false path. Found {:?}.", n),
                    }

                    Ok(Some(self.graph.after_loop_foot(false_path)))
                } else {
                    // Branch condition
                    self.passenger.branch_condition(current, condition)?;

                    let (true_path, false_path) = self.graph.after_condition(current);

                    self.passenger.branch_start_true_path(true_path)?;

                    let mut merge = None;

                    // True path
                    let mut current_node = true_path;
                    for _ in 0..self.node_count {
                        match *self.graph.node_weight(current_node) {
                            Node::BranchMerge(id) => {
                                self.passenger.branch_end_true_path(current_node)?;
                                merge = Some(current_node);
                                break;
                            }

                            _ => (),
                        }

                        match self.visit_node(current_node)? {
                            Some(next) => current_node = next,
                            None => return Ok(None),
                        }
                    }

                    if merge.is_none() {
                        panic!("Traversed entire graph and did not find Condition::BranchMerge");
                    }

                    self.passenger.branch_start_false_path(false_path)?;

                    // False path
                    let mut current_node = false_path;
                    let mut merge = None;
                    for _ in 0..self.node_count {
                        match *self.graph.node_weight(current_node) {
                            Node::BranchMerge(id) => {
                                self.passenger.branch_end_false_path(current_node)?;
                                merge = Some(current_node);
                                break;
                            }

                            _ => (),
                        }

                        match self.visit_node(current_node)? {
                            Some(next) => current_node = next,
                            None => return Ok(None),
                        }
                    }

                    if merge.is_none() {
                        panic!("Traversed entire graph and did not find Condition::BranchMerge");
                    }

                    Ok(Some(self.graph.next(merge.unwrap())))
                }
            }
        }
    }
}