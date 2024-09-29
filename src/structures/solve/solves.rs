use crate::procedures::hobson_choices;
use crate::structures::solve::{Solve, SolveError, SolveOk};
use crate::structures::{
    stored_clause, Clause, ClauseStatus, Literal, LiteralSource, StoredClause, Valuation,
};
use std::borrow::BorrowMut;
use std::rc::Rc;

pub enum SolveResult {
    Satisfiable,
    Unsatisfiable,
    Unknown,
}

pub struct SolveStats {
    pub total_time: std::time::Duration,
    pub examination_time: std::time::Duration,
    pub implication_time: std::time::Duration,
    pub unsat_time: std::time::Duration,
    pub reduction_time: std::time::Duration,
    pub choice_time: std::time::Duration,
    pub iterations: usize,
    pub conflicts: usize,
}

impl SolveStats {
    pub fn new() -> Self {
        SolveStats {
            total_time: std::time::Duration::new(0, 0),
            examination_time: std::time::Duration::new(0, 0),
            implication_time: std::time::Duration::new(0, 0),
            unsat_time: std::time::Duration::new(0, 0),
            reduction_time: std::time::Duration::new(0, 0),
            choice_time: std::time::Duration::new(0, 0),
            iterations: 0,
            conflicts: 0,
        }
    }
}

impl std::fmt::Display for SolveStats {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "c STATS")?;
        writeln!(f, "c ITERATIONS: {}", self.iterations)?;
        writeln!(f, "c CONFLICTS: {}", self.conflicts)?;
        writeln!(f, "c TIME: {:.2?}", self.total_time)?;
        writeln!(f, "c \tEXAMINATION: {:.2?}", self.examination_time)?;
        writeln!(f, "c \tIMPLICATION: {:.2?}", self.implication_time)?;
        writeln!(f, "c \tUNSAT: {:.2?}", self.unsat_time)?;
        writeln!(f, "c \tREDUCTION: {:.2?}", self.reduction_time)?;
        writeln!(f, "c \tCHOICE: {:.2?}", self.choice_time)?;
        Ok(())
    }
}

impl Solve<'_> {
    pub fn implication_solve(&mut self) -> (SolveResult, SolveStats) {
        let this_total_time = std::time::Instant::now();

        let mut stats = SolveStats::new();

        self.set_from_lists(hobson_choices(self.clauses())); // settle any literals which occur only as true or only as false

        let result: SolveResult;

        'main_loop: loop {
            stats.iterations += 1;

            log::trace!("Loop on valuation: {}", self.valuation.as_internal_string());

            let this_examination_time = std::time::Instant::now();

            stats.examination_time += this_examination_time.elapsed();

            let mut some_deduction = false;
            let mut some_conflict = false;

            if self.current_level().get_choice().is_some() {
                let literals = self.levels[self.current_level().index()]
                    .updated_watches()
                    .clone();

                let mut conflicts_to_process = vec![];

                for literal in literals {
                    for i in 0..self.variables[literal.v_id].watch_occurrences().len() {
                        let stored_clause =
                            self.variables[literal.v_id].watch_occurrences()[i].clone();
                        let clause_status = stored_clause.watch_choices(&self.valuation);

                        match clause_status {
                            ClauseStatus::Entails(consequent) => {
                                let this_implication_time = std::time::Instant::now();
                                let set_result = self.set_literal(
                                    consequent,
                                    LiteralSource::StoredClause(stored_clause.clone()),
                                );
                                match set_result {
                                    Err(SolveError::Conflict(_, _)) => {
                                        some_conflict = true;
                                    }
                                    Err(e) => panic!(
                                        "Unexpected error {e:?} when setting literal {consequent}"
                                    ),
                                    Ok(()) => {
                                        some_deduction = true;
                                    }
                                }
                                stats.implication_time += this_implication_time.elapsed();
                            }
                            ClauseStatus::Conflict => conflicts_to_process.push(stored_clause),
                            ClauseStatus::Unsatisfied => (),
                            ClauseStatus::Satisfied => (),
                        }
                    }
                }

                for stored_conflict in conflicts_to_process {
                    match process_conflict(self, &stored_conflict, &mut stats) {
                        ProcessOption::ContinueMain => {
                            if true {
                                continue 'main_loop;
                            }
                        }
                        ProcessOption::Unsatisfiable => {
                            stats.total_time = this_total_time.elapsed();

                            return (SolveResult::Unsatisfiable, stats);
                        }
                        _ => panic!("unexp@ected"),
                    }
                }
            } else {
                let mut conflicts_to_process = vec![];

                for i in 0..self.formula_clauses.len() {
                    let stored_clause = self.formula_clauses[i].clone();
                    let clause_status = stored_clause.watch_choices(&self.valuation);

                    match clause_status {
                        ClauseStatus::Entails(consequent) => {
                            let this_implication_time = std::time::Instant::now();
                            match self.set_literal(
                                consequent,
                                LiteralSource::StoredClause(stored_clause.clone()),
                            ) {
                                Err(SolveError::Conflict(_, _)) => {
                                    some_conflict = true;
                                }
                                Err(e) => panic!(
                                    "Unexpected error {e:?} when setting literal {consequent}"
                                ),
                                Ok(()) => {
                                    some_deduction = true;
                                }
                            }
                            stats.implication_time += this_implication_time.elapsed();
                        }
                        ClauseStatus::Conflict => conflicts_to_process.push(stored_clause),
                        ClauseStatus::Unsatisfied => (),
                        ClauseStatus::Satisfied => (),
                    }
                }

                for i in 0..self.learnt_clauses.len() {
                    let stored_clause = self.learnt_clauses[i].clone();
                    let clause_status = stored_clause.watch_choices(&self.valuation);

                    match clause_status {
                        ClauseStatus::Entails(consequent) => {
                            let this_implication_time = std::time::Instant::now();
                            match self.set_literal(
                                consequent,
                                LiteralSource::StoredClause(stored_clause.clone()),
                            ) {
                                Err(SolveError::Conflict(_, _)) => {
                                    some_conflict = true;
                                }
                                Err(e) => panic!(
                                    "Unexpected error {e:?} when setting literal {consequent}"
                                ),
                                Ok(()) => {
                                    some_deduction = true;
                                }
                            }
                            stats.implication_time += this_implication_time.elapsed();
                        }
                        ClauseStatus::Conflict => conflicts_to_process.push(stored_clause),
                        ClauseStatus::Unsatisfied => (),
                        ClauseStatus::Satisfied => (),
                    }
                }

                for stored_conflict in conflicts_to_process {
                    match process_conflict(self, &stored_conflict, &mut stats) {
                        ProcessOption::ContinueMain => {
                            if true {
                                continue 'main_loop;
                            }
                        }
                        ProcessOption::Unsatisfiable => {
                            result = SolveResult::Unsatisfiable;
                            break 'main_loop;
                        }
                        _ => panic!("unexp@ected"),
                    }
                }
            }

            if !(some_conflict || some_deduction) {
                if let Some(available_v_id) = self.most_active_none(&self.valuation) {
                    if self.time_to_reduce() {
                        reduce(self, &mut stats)
                    }

                    let this_choice_time = std::time::Instant::now();
                    log::trace!(
                        "Choice: {available_v_id} @ {} with activity {}",
                        self.current_level().index(),
                        self.variables[available_v_id].activity()
                    );

                    let _ = self
                        .set_literal(Literal::new(available_v_id, false), LiteralSource::Choice);
                    stats.choice_time += this_choice_time.elapsed();

                    continue 'main_loop;
                } else {
                    result = SolveResult::Satisfiable;
                    break 'main_loop;
                }
            }
        }

        stats.total_time = this_total_time.elapsed();
        match result {
            SolveResult::Satisfiable => {
                println!(
                    "c ASSIGNMENT: {}",
                    self.valuation.to_vec().as_display_string(self)
                );
                return (SolveResult::Satisfiable, stats);
            }
            SolveResult::Unsatisfiable => {}
            SolveResult::Unknown => {}
        }
        (result, stats)
    }
}

// #[inline(always)]
fn reduce(solve: &mut Solve, stats: &mut SolveStats) {
    let this_reduction_time = std::time::Instant::now();
    println!("time to reduce");
    solve.learnt_clauses.sort_unstable_by_key(|a| a.lbd());

    let learnt_count = solve.learnt_clauses.len();
    println!("Learnt count: {}", learnt_count);
    for _ in 0..learnt_count {
        if solve
            .learnt_clauses
            .last()
            .is_some_and(|lc| lc.lbd() > solve.config.min_glue_strength)
        {
            let goodbye = solve.learnt_clauses.last().unwrap().clone();
            solve.drop_clause(&goodbye);
        } else {
            break;
        }
    }
    solve.forgets += 1;
    solve.conflcits_since_last_forget = 0;
    stats.reduction_time += this_reduction_time.elapsed();
    println!("Reduced to: {}", solve.learnt_clauses.len());
}

enum ProcessOption {
    Unsatisfiable,
    ContinueMain,
    Implicationed,
    Conflict,
}

fn process_clause(
    solve: &mut Solve,
    stored_clause: &Rc<StoredClause>,
    clause_status: &ClauseStatus,
    stats: &mut SolveStats,
    some_conflict: &mut bool,
    some_deduction: &mut bool,
) -> ProcessOption {
    match clause_status {
        ClauseStatus::Entails(consequent) => {
            let this_implication_time = std::time::Instant::now();
            match solve.set_literal(
                *consequent,
                LiteralSource::StoredClause(stored_clause.clone()),
            ) {
                Err(SolveError::Conflict(_, _)) => {
                    *some_conflict = true;
                }
                Err(e) => panic!("Unexpected error {e:?} when setting literal {consequent}"),
                Ok(()) => {
                    *some_deduction = true;
                }
            }
            stats.implication_time += this_implication_time.elapsed();
            ProcessOption::Implicationed
        }
        ClauseStatus::Conflict => ProcessOption::Conflict,
        _ => panic!("Something unexpected"),
    }
}

fn process_conflict(
    solve: &mut Solve,
    stored_conflict: &Rc<StoredClause>,
    stats: &mut SolveStats,
) -> ProcessOption {
    let this_unsat_time = std::time::Instant::now();
    solve.notice_conflict(stored_conflict);
    stats.conflicts += 1;
    match solve.attempt_fix(stored_conflict.clone()) {
        Err(SolveError::NoSolution) => {
            if solve.config.core {
                solve.core();
            }
            ProcessOption::Unsatisfiable
        }
        Ok(SolveOk::AssertingClause) | Ok(SolveOk::Deduction(_)) => {
            stats.unsat_time += this_unsat_time.elapsed();
            ProcessOption::ContinueMain
        }
        Ok(ok) => panic!("Unexpected ok {ok:?} when attempting a fix"),
        Err(err) => {
            panic!("Unexpected error {err:?} when attempting a fix")
        }
    }
}
