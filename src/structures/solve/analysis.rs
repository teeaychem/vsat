use crate::procedures::{find_counterpart_literals, resolve_sorted_clauses};
use crate::structures::solve::{Solve, SolveError, SolveOk};
use crate::structures::{
    stored_clause::initialise_watches_for, Clause, ClauseSource, LiteralSource, StoredClause,
    Valuation,
};

use std::rc::Rc;

pub enum AnalysisResult {
    AssertingClause(Rc<StoredClause>),
}

impl Solve<'_> {
    /// Either the most recent decision level in the resolution clause prior to the current level or 0.
    fn decision_level(&self, stored_clause: &Rc<StoredClause>) -> usize {
        let mut levels = self
            .decision_levels_of(stored_clause.clause())
            .collect::<Vec<_>>();
        levels.sort_unstable();
        levels.dedup();
        levels.reverse();
        if levels.len() > 1 {
            levels[1]
        } else {
            0
        }
    }

    pub fn attempt_fix(
        &mut self,
        conflict_clause: Rc<StoredClause>,
    ) -> Result<SolveOk, SolveError> {
        let the_id = conflict_clause.id();
        log::warn!(
            "Attempt to fix on clause {the_id} at level {}",
            self.current_level().index()
        );
        match self.current_level().index() {
            0 => Err(SolveError::NoSolution),
            _ => match self.analysis_switch(conflict_clause) {
                AnalysisResult::AssertingClause(asserting_clause) => {
                    let backjump_level = self.decision_level(&asserting_clause);

                    initialise_watches_for(
                        &asserting_clause,
                        &self.valuation_at(backjump_level),
                        &mut self.variables,
                    );

                    self.backjump(backjump_level);

                    Ok(SolveOk::AssertingClause)
                }
            },
        }
    }

    pub fn analysis_switch(&mut self, conflict_clause: Rc<StoredClause>) -> AnalysisResult {
        match self.config.analysis {
            1 => self.simple_analysis_one(conflict_clause),
            2 => self.simple_analysis_two(conflict_clause),
            3 => self.simple_analysis_three(conflict_clause),
            _ => panic!("Unknown analysis"),
        }
    }

    /// Simple analysis performs resolution on any clause used to obtain a conflict literal at the current decision level.
    pub fn simple_analysis_one(&mut self, stored_clause: Rc<StoredClause>) -> AnalysisResult {
        let mut resolution_history = vec![];
        let mut the_resolved_clause = stored_clause.clause().as_vec();

        'resolution_loop: loop {
            log::trace!("Analysis clause: {}", the_resolved_clause.as_string());
            // the current choice will never be a resolution literal, as these are those literals in the clause which are the result of propagation
            let mut resolution_literals = self
                .implication_graph
                .resolution_candidates_at_level(&the_resolved_clause, self.current_level().index())
                .collect::<Vec<_>>();
            resolution_literals.sort_unstable();
            resolution_literals.dedup();

            if let Some((stored_clause, resolution_literal)) = resolution_literals.first() {
                resolution_history.push(stored_clause.clone());
                the_resolved_clause = resolve_sorted_clauses(
                    &the_resolved_clause.to_vec(),
                    &stored_clause.clause().as_vec(),
                    resolution_literal.v_id,
                )
                .expect("Resolution failed")
                .as_vec();

                continue 'resolution_loop;
            } else {
                break 'resolution_loop;
            }
        }

        let sc = self.store_clause(the_resolved_clause, ClauseSource::Resolution);
        self.resolution_graph
            .add_resolution(resolution_history.iter(), &sc);

        AnalysisResult::AssertingClause(sc)
    }

    pub fn simple_analysis_two(&mut self, stored_clause: Rc<StoredClause>) -> AnalysisResult {
        log::warn!("Simple analysis two");
        log::warn!("The valuation is: {}", self.valuation.as_internal_string());

        let the_conflict_clause = stored_clause;
        log::warn!(
            "Simple analysis two on: {}",
            the_conflict_clause.clause().as_string()
        );

        let mut the_resolved_clause = the_conflict_clause.clause().as_vec();
        let the_conflict_level_choice = {
            let conflict_decision_level = self
                .decision_levels_of(the_conflict_clause.clause())
                .max()
                .expect("No clause decision level");
            self.level_choice(conflict_decision_level)
        };

        let the_immediate_domiator = self
            .implication_graph
            .immediate_dominators(the_resolved_clause.literals(), the_conflict_level_choice)
            .expect("No immediate dominator");

        log::warn!("Resolution on paths…");

        let mut resolution_history = vec![];

        for literal in the_conflict_clause.literals() {
            match self
                .implication_graph
                .some_clause_path_between(the_immediate_domiator, literal.negate())
            {
                None => continue,
                Some(mut path_clauses) => {
                    path_clauses.reverse(); // Not strictly necessary
                    for path_clause in path_clauses {
                        if let Some(shared_literal) =
                            path_clause.clause().literals().find(|path_literal| {
                                the_resolved_clause.contains(&path_literal.negate())
                            })
                        {
                            resolution_history.push(path_clause.clone());
                            the_resolved_clause = resolve_sorted_clauses(
                                &the_resolved_clause,
                                &path_clause.clause().as_vec(),
                                shared_literal.v_id,
                            )
                            .expect("Resolution failed")
                            .as_vec();
                        };
                    }
                }
            }
        }

        let sc = self.store_clause(the_resolved_clause, ClauseSource::Resolution);
        self.resolution_graph
            .add_resolution(resolution_history.iter(), &sc);

        AnalysisResult::AssertingClause(sc)
    }

    pub fn simple_analysis_three(&mut self, conflict_clause: Rc<StoredClause>) -> AnalysisResult {
        let mut resolved_clause = conflict_clause.clause().as_vec();
        let mut resolution_trail = vec![];
        let mut observations = self.current_level().observations().collect::<Vec<_>>();
        observations.reverse();
        let resolution_possibilites = observations.into_iter().filter_map(|(src, lit)| match src {
            LiteralSource::StoredClause(cls) => Some((cls, lit)),
            _ => None,
        });
        let previous_level_val = self.valuation_at(self.current_level().index() - 1);

        for (src, _lit) in resolution_possibilites {
            if resolved_clause.asserts(&previous_level_val).is_some() {
                break;
            }

            let src_cls_vec = src.clause().as_vec();
            let counterparts = find_counterpart_literals(&resolved_clause, &src_cls_vec);

            if let Some(counterpart) = counterparts.first() {
                resolution_trail.push(src.clone());
                resolved_clause =
                    resolve_sorted_clauses(&resolved_clause, &src_cls_vec, *counterpart)
                        .unwrap()
                        .as_vec()
            }
        }

        let sc = self.store_clause(resolved_clause, ClauseSource::Resolution);
        self.resolution_graph
            .add_resolution(resolution_trail.iter(), &sc);

        AnalysisResult::AssertingClause(sc)
    }

    pub fn core(&self) {
        println!("An unsatisfiable core of the input formula:");
        let node_indicies =
            self.top_level()
                .observations()
                .filter_map(|(source, _)| match source {
                    LiteralSource::StoredClause(stored_clause) => Some(stored_clause.nx()),
                    _ => None,
                });
        let node_indicies_vec = node_indicies.collect::<Vec<_>>();
        let simple_core = self.resolution_graph.extant_origins(node_indicies_vec);
        for clause in simple_core {
            println!("{}", clause.clause().as_string())
        }
    }
}
