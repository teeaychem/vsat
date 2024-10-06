use crate::structures::{
    clause::{Clause, ClauseId, ClauseVec},
    literal::Literal,
    valuation::{Valuation, ValuationVec},
    variable::{Variable, VariableId},
};

use std::cell::Cell;

#[derive(Clone, Debug)]
pub enum ClauseSource {
    Formula,
    Resolution(Vec<ClauseKey>),
}

#[derive(Debug, Clone, Copy)]
pub enum Watch {
    A,
    B,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClauseKey {
    Formula(slotmap::DefaultKey),
    Learnt(slotmap::DefaultKey),
}

/**
The stored clause struct associates a clause with metadata relevant for a solve
and, is intended to be the unique representation of a clause within a solve
- `lbd` is the literal block distance of the clause
  - note, this defaults to 0 and should be updated if a clause is stored after some decisions have been made
- `watch_a` and `watch_b` are pointers to the watched literals, and rely on a vector representation of the clause
  - note, both default to 0 and should be initialised with `initialise_watches_for` when the clause is stored
*/
#[derive(Debug)]
pub struct StoredClause {
    id: ClauseId,
    pub key: ClauseKey,
    lbd: Cell<usize>,
    source: ClauseSource,
    clause: ClauseVec,
    watch_a: Cell<usize>,
    watch_b: Cell<usize>,
}

#[derive(Debug)]
pub enum ClauseStatus {
    Satisfied,        // some watch literal matches
    Conflict,         // no watch literals matches
    Entails(Literal), // Literal is unassigned and the no other watch matches
    Unsatisfied,      // more than one literal is unassigned
}

/// The already/new variants allow for contrast with a known previous state
#[derive(PartialEq, Debug)]
pub enum WatchStatus {
    AlreadyConflict,
    AlreadyImplication,
    AlreadySatisfied,
    NewImplication,
    NewSatisfied,
    NewTwoNone,
}

/// The value is used to suggest an updated index
#[derive(Debug)]
pub enum WatchUpdateEnum {
    Witness(usize),
    None(usize),
    No,
}

impl StoredClause {
    pub fn new_from(
        id: ClauseId,
        key: ClauseKey,
        clause: impl Clause,
        source: ClauseSource,
    ) -> StoredClause {
        if clause.is_empty() {
            panic!("An empty clause")
        }

        StoredClause {
            id,
            key,
            lbd: Cell::new(0),
            clause: clause.to_vec(),
            source,
            watch_a: Cell::from(0),
            watch_b: Cell::from(0),
        }
    }

    pub fn id(&self) -> ClauseId {
        self.id
    }

    pub fn source(&self) -> &ClauseSource {
        &self.source
    }

    pub fn clause_impl(&self) -> &impl Clause {
        &self.clause
    }

    pub fn clause(&self) -> &[Literal] {
        &self.clause
    }

    pub fn literal_at(&self, position: usize) -> Literal {
        self.clause[position]
    }

    pub fn literals(&self) -> impl Iterator<Item = Literal> + '_ {
        self.clause.literals()
    }

    fn index_of(&self, vid: VariableId) -> usize {
        self.clause
            .iter()
            .enumerate()
            .find(|(_, l)| l.v_id == vid)
            .map(|(idx, _)| idx)
            .expect("Literal not found in clause")
    }

    pub fn get_watch(&self, a_or_b: Watch) -> usize {
        match a_or_b {
            Watch::A => self.watch_a.get(),
            Watch::B => self.watch_b.get(),
        }
    }
}

impl StoredClause {
    /// Find the index of a literal which has not been valued, if possible, else if there was some witness for the clause, return that
    pub fn some_none_or_else_witness_idx(
        &self,
        val: &impl Valuation,
        but_not: VariableId,
    ) -> WatchUpdateEnum {
        let mut witness = None;
        for (idx, literal) in self.clause.iter().enumerate() {
            if but_not != literal.v_id {
                if let Some(val) = val.of_v_id(literal.v_id) {
                    if val == literal.polarity {
                        witness = Some(idx);
                    }
                } else {
                    return WatchUpdateEnum::None(idx);
                }
            }
        }
        match witness {
            Some(idx) => WatchUpdateEnum::Witness(idx),
            None => WatchUpdateEnum::No,
        }
    }

    /// Finds an index of the clause vec whose value is None on val and differs from but_not.
    fn some_none_idx(&self, val: &impl Valuation, but_not: Option<VariableId>) -> Option<usize> {
        self.clause
            .iter()
            .enumerate()
            .find(|(_, l)| {
                let excluded = if let Some(to_exclude) = but_not {
                    l.v_id != to_exclude
                } else {
                    true
                };
                excluded && val.of_v_id(l.v_id).is_none()
            })
            .map(|(idx, _)| idx)
    }

    /// Finds an index of the clause vec which witness the clause is true on val and differs from but_not.
    fn some_witness_index(
        &self,
        val: &impl Valuation,
        but_not: Option<VariableId>,
    ) -> Option<usize> {
        self.clause
            .iter()
            .enumerate()
            .find(|(_, l)| {
                let excluded = if let Some(to_exclude) = but_not {
                    l.v_id != to_exclude
                } else {
                    true
                };
                let polarity_match = val.of_v_id(l.v_id).is_some_and(|v| v == l.polarity);
                excluded && polarity_match
            })
            .map(|(idx, _)| idx)
    }

    /// Finds an index of the clause vec which witness the clause is false on val and differs from but_not.
    /// And, in particular, ensures the decision level of the variable corresponding to the index is as high as possible.
    /*
    By ensuring the decision level of the variable is as high as possible we guarantee that the watch pair is only revised from some to none if the solve backtracks from the decision level of the watch.
     */
    fn some_differing_index(
        &self,
        val: &impl Valuation,
        but_not: Option<VariableId>,
        vars: &[Variable],
    ) -> Option<usize> {
        let (mut index, mut level) = (None, 0);

        for (i, l) in self.clause.iter().enumerate() {
            if val.of_v_id(l.v_id).is_some_and(|val_polarity| {
                (val_polarity != l.polarity
                    && (index.is_none() || level < vars[l.v_id].decision_level().unwrap()))
                    && (but_not.is_none() || but_not.is_some_and(|vid| l.v_id != vid))
            }) {
                (index, level) = (Some(i), vars[l.v_id].decision_level().unwrap());
            }
        }

        index
    }

    /// Finds some index of the clause vec which isn't but_not with the preference:
    ///   A. The index points to a literal which is true on val.
    ///   B. The index points to a literal which is unassigned on val.
    ///   C. The index points to a literal which is false on val.
    /// This preference contributes to maintaining useful watch literals.
    /// As, it is essentail to know when a clause is true, as it then can provide no useful information.
    /// And, if a watch is only on a differing literal when there are no other unassigned literals
    /// it follows the other watched literal must be true on the valuation, or else there's a contradiction.
    fn some_preferred_index(
        &self,
        val: &impl Valuation,
        but_not: Option<usize>,
        vars: &[Variable],
    ) -> usize {
        if let Some(index) = self.some_witness_index(val, but_not) {
            index
        } else if let Some(index) = self.some_none_idx(val, but_not) {
            index
        } else if let Some(index) = self.some_differing_index(val, but_not, vars) {
            index
        } else {
            panic!("Could not find a suitable index");
        }
    }
}

impl StoredClause {
    pub fn watch_choices(&self, val: &impl Valuation) -> ClauseStatus {
        let a_literal = self.clause[self.watch_a.get()];
        let a_val = val.of_v_id(a_literal.v_id);

        match self.clause.len() {
            1 => match a_val {
                // both watches point to the only literal
                Some(polarity) if polarity == a_literal.polarity => ClauseStatus::Satisfied,
                Some(_) => ClauseStatus::Conflict,
                None => ClauseStatus::Entails(a_literal),
            },
            _ => {
                let b_literal = self.clause[self.watch_b.get()];

                let b_val = val.of_v_id(b_literal.v_id);

                if a_val.is_none() && b_val.is_none() {
                    ClauseStatus::Unsatisfied
                } else if a_val.is_some_and(|p| p == a_literal.polarity)
                    || b_val.is_some_and(|p| p == b_literal.polarity)
                {
                    ClauseStatus::Satisfied
                } else if b_val.is_none() {
                    ClauseStatus::Entails(b_literal)
                } else if a_val.is_none() {
                    ClauseStatus::Entails(a_literal)
                } else {
                    // if a_val.is_some_and(|p_a| { p_a != a_literal.polarity && b_val.is_some_and(|p_b| p_b != b_literal.polarity)}) {
                    ClauseStatus::Conflict
                }

                // panic!("Unexpected combination of watch literals")
            }
        }
    }

    pub fn set_lbd(&self, vars: &[Variable]) {
        self.lbd.set(self.clause.lbd(vars));
    }

    pub fn lbd(&self) -> usize {
        self.lbd.get()
    }

    pub fn watched_a(&self) -> Literal {
        self.clause[self.watch_a.get()]
    }

    pub fn watched_b(&self) -> Literal {
        self.clause[self.watch_b.get()]
    }

    pub fn update_watch_a(&self, val: usize) {
        self.watch_a.set(val);
    }

    pub fn update_watch_b(&self, val: usize) {
        self.watch_b.set(val);
    }

    pub fn clause_clone(&self) -> ClauseVec {
        self.clause.clone()
    }
}

/// Initialises the watches for a stored clause, is not a method as requires pointer information
pub fn initialise_watches_for(
    stored_clause: &StoredClause,
    val: &impl Valuation,
    vars: &[Variable],
) {
    if stored_clause.clause.len() > 1 {
        stored_clause
            .watch_a
            .set(stored_clause.some_preferred_index(val, None, vars));

        stored_clause.watch_b.set({
            let literal_a = stored_clause.clause[stored_clause.watch_a.get()];
            stored_clause.some_preferred_index(val, Some(literal_a.v_id), vars)
        });

        let current_a = stored_clause.clause[stored_clause.watch_a.get()];
        vars[current_a.v_id].watch_added(stored_clause.key, current_a.polarity);

        let current_b = stored_clause.clause[stored_clause.watch_b.get()];
        vars[current_b.v_id].watch_added(stored_clause.key, current_b.polarity);
    } else {
        let watched_variable = stored_clause.clause.first().unwrap();
        vars[watched_variable.v_id].watch_added(stored_clause.key, watched_variable.polarity);
    }
}

// #[rustfmt::skip]
/// Updates the two watched literals on the assumption that only the valuation of the given id has changed.
pub fn relic_suggest_watch_update(
    stored_clause: &StoredClause,
    val: &impl Valuation,
    v_id: VariableId,
    vars: &[Variable],
) -> (Option<usize>, Option<usize>, WatchStatus) {
    match stored_clause.length() {
        1 => match val.of_v_id(stored_clause.clause[stored_clause.watch_a.get()].v_id) {
            None => (None, None, WatchStatus::AlreadyImplication),
            Some(_) => (None, None, WatchStatus::AlreadySatisfied),
        },
        _ => {
            // If the current a watch already witness satisfaction of the clause, do nothing
            let watched_a_literal = stored_clause.clause[stored_clause.watch_a.get()];
            let current_a_value = val.of_v_id(watched_a_literal.v_id);
            let current_a_match = current_a_value.is_some_and(|p| p == watched_a_literal.polarity);
            if current_a_match {
                return (None, None, WatchStatus::AlreadySatisfied);
            }
            // and likewise for the current b watch
            let watched_b_literal = stored_clause.clause[stored_clause.watch_b.get()];
            let current_b_value = val.of_v_id(watched_b_literal.v_id);
            let current_b_match = current_b_value.is_some_and(|p| p == watched_b_literal.polarity);
            if current_b_match {
                return (None, None, WatchStatus::AlreadySatisfied);
            }
            // as, the decision level of the witnessing literal must be lower than that of the current literal

            let clause_literal_index = stored_clause.index_of(v_id);

            // check to see if the clause is satisfied, if so, the previous two checks imply one watch must be updated to witness the satisfaction
            let clause_is_satisfied_by_v = {
                let valuation_polarity = val.of_v_id(v_id).unwrap();
                let clause_polarity = stored_clause.find_literal_by_id(v_id).unwrap().polarity;
                valuation_polarity == clause_polarity
            };

            if clause_is_satisfied_by_v {
                // attempt to update a watch which doesn't interact with the current valuation
                if current_a_value.is_none() {
                    return (Some(clause_literal_index), None, WatchStatus::NewSatisfied);
                } else if current_b_value.is_none() {
                    return (None, Some(clause_literal_index), WatchStatus::NewSatisfied);
                } else {
                    // otherwise, both literals must conflict with the current valuation, so update the most recent
                    if vars[watched_a_literal.v_id]
                        .decision_level()
                        .expect("No decision level for watch a")
                        > vars[watched_b_literal.v_id]
                            .decision_level()
                            .expect("No decision level for watch b")
                    {
                        return (Some(clause_literal_index), None, WatchStatus::NewSatisfied);
                    } else {
                        return (None, Some(clause_literal_index), WatchStatus::NewSatisfied);
                    }
                }
            }

            // otherwise, if either watch conflicts with the current valuation,
            // an attempt should be made to avoid the conflict
            // as both watches must be different, order is irrelvant here
            if watched_a_literal.v_id == v_id
                && current_a_value.is_some_and(|p| p != watched_a_literal.polarity)
            {
                if let Some(idx) = stored_clause.some_none_idx(val, Some(watched_b_literal.v_id)) {
                    // and, there's no literal on the watch which doesn't have a value on the assignment
                    match current_b_match {
                        false => (Some(idx), None, WatchStatus::NewImplication),
                        true => (Some(idx), None, WatchStatus::NewTwoNone),
                    }
                } else {
                    (None, None, WatchStatus::AlreadyConflict)
                }
            } else if watched_b_literal.v_id == v_id
                && current_b_value.is_some_and(|p| p != watched_b_literal.polarity)
            {
                if let Some(idx) = stored_clause.some_none_idx(val, Some(watched_a_literal.v_id)) {
                    match current_a_match {
                        false => (None, Some(idx), WatchStatus::NewImplication),
                        true => (None, Some(idx), WatchStatus::NewTwoNone),
                    }
                } else {
                    (None, None, WatchStatus::AlreadyConflict)
                }
            } else {
                (None, None, WatchStatus::AlreadyConflict)
            }
        }
    }
}

impl std::fmt::Display for StoredClause {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "#[{}] {}", self.id, self.clause.as_string())
    }
}

impl PartialOrd for StoredClause {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for StoredClause {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialEq for StoredClause {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for StoredClause {}

/// Lift the method from the clause stored to the stored clause
impl Clause for StoredClause {
    fn literals(&self) -> impl Iterator<Item = Literal> {
        self.clause.literals()
    }

    fn variables(&self) -> impl Iterator<Item = VariableId> {
        self.clause.variables()
    }

    fn is_sat_on(&self, valuation: &ValuationVec) -> bool {
        self.clause.is_sat_on(valuation)
    }

    fn is_unsat_on(&self, valuation: &ValuationVec) -> bool {
        self.clause.is_unsat_on(valuation)
    }

    fn find_unit_literal<T: Valuation>(&self, valuation: &T) -> Option<Literal> {
        self.clause.find_unit_literal(valuation)
    }

    fn collect_choices<T: Valuation>(&self, valuation: &T) -> Option<Vec<Literal>> {
        self.clause.collect_choices(valuation)
    }

    fn as_string(&self) -> String {
        self.clause.as_string()
    }

    fn as_dimacs(&self, variables: &[Variable]) -> String {
        self.clause.as_dimacs(variables)
    }

    fn is_empty(&self) -> bool {
        self.clause.is_empty()
    }

    fn to_vec(self) -> ClauseVec {
        self.clause
    }

    fn length(&self) -> usize {
        self.clause.len()
    }

    fn asserts(&self, val: &impl Valuation) -> Option<Literal> {
        self.clause.asserts(val)
    }

    fn lbd(&self, variables: &[Variable]) -> usize {
        self.clause.lbd(variables)
    }

    fn find_literal_by_id(&self, id: VariableId) -> Option<Literal> {
        self.clause.find_literal_by_id(id)
    }
}
