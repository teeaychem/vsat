use crate::{
    structures::{Literal, LiteralError, Valuation, ValuationVec},
    Assignment,
};

pub type ClauseId = usize;

#[derive(Debug)]
pub enum ClauseError {
    Literal(LiteralError),
    Empty,
}

#[derive(Debug)]
pub struct Clause {
    pub id: usize,
    pub literals: Vec<Literal>,
}

impl std::fmt::Display for Clause {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "#[{}] ", self.id)?;
        write!(f, "(")?;
        for literal in self.literals.iter() {
            write!(f, " {literal} ")?;
        }
        write!(f, ")")?;
        Ok(())
    }
}

impl Clause {
    pub fn new(position: usize) -> Clause {
        Clause {
            id: position,
            literals: Vec::new(),
        }
    }

    pub fn add_literal(&mut self, literal: Literal) -> Result<(), ClauseError> {
        self.literals.push(literal);
        Ok(())
    }

    pub fn is_sat_on(&self, assignment: &ValuationVec) -> bool {
        self.literals
            .iter()
            .any(|l| assignment.of_v_id(l.v_id) == Ok(Some(l.polarity)))
    }

    pub fn is_unsat_on(&self, assignment: &ValuationVec) -> bool {
        self.literals.iter().all(|l| {
            if let Ok(Some(variable_assignment)) = assignment.of_v_id(l.v_id) {
                variable_assignment != l.polarity
            } else {
                false
            }
        })
    }

    pub fn find_unit_literal<T: Valuation>(&self, assignment: &T) -> Option<Literal> {
        let mut unit = None;

        for literal in &self.literals {
            if let Ok(assigned_value) = assignment.of_v_id(literal.v_id) {
                if assigned_value.is_some_and(|v| v == literal.polarity) {
                    // the clause is satisfied and so does not provide any new information
                    break;
                } else if assigned_value.is_some() {
                    // either every literal so far has been valued the opposite, or there has been exactly on unvalued literal, so continue
                    continue;
                } else {
                    // if no other literal has been found then this literal may be unit, so mark it and continue
                    // though, if some other literal has already been marked, the clause does not force any literal
                    match unit {
                        Some(_) => {
                            unit = None;
                            break;
                        }
                        None => unit = Some(literal.clone()),
                    }
                }
            }
        }
        unit
    }
}
