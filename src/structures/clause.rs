use crate::{
    literal,
    structures::{Assignment, Literal, LiteralError},
};

pub type ClauseId = usize;

#[derive(Debug)]
pub enum ClauseError {
    Literal(LiteralError),
    Empty,
}

#[derive(Debug)]
pub struct Clause {
    id: Option<ClauseId>,
    literals: Vec<Literal>,
}

impl std::fmt::Display for Clause {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(id) = self.id {
            write!(f, "#[{id}] ")?;
        }
        write!(f, "(")?;
        for literal in self.literals.iter() {
            write!(f, " {literal} ")?;
        }
        write!(f, ")")?;
        Ok(())
    }
}

impl Clause {
    pub fn new() -> Clause {
        Clause {
            id: None,
            literals: Vec::new(),
        }
    }

    pub fn literals(&self) -> &Vec<Literal> {
        &self.literals
    }

    pub fn id(&self) -> Option<ClauseId> {
        self.id
    }

    pub fn set_id(&mut self, to: ClauseId) {
        self.id = Some(to);
    }

    pub fn add_literal(&mut self, literal: Literal) -> Result<(), ClauseError> {
        self.literals.push(literal);
        Ok(())
    }

    pub fn is_sat_on(&self, assignment: &Assignment) -> bool {
        self.literals
            .iter()
            .any(|l| assignment.get(l.variable()) == Some(Some(l.polarity())))
    }

    pub fn is_unsat_on(&self, assignment: &Assignment) -> bool {
        self.literals.iter().all(|l| {
            if let Some(Some(variable_assignment)) = assignment.get(l.variable()) {
                variable_assignment != l.polarity()
            } else {
                false
            }
        })
    }

    pub fn get_unit_on(&self, assignment: &Assignment) -> Option<(Literal, ClauseId)> {
        let mut unit = None;
        for literal in &self.literals {
            if let Some(assignment) = assignment.get(literal.variable()) {
                match assignment {
                    Some(true) => break,     // as the clause does not provide any new information
                    Some(false) => continue, // some other literal must be true
                    None => {
                        // if no assignment, then literal must be true…
                        match unit {
                            Some(_) => {
                                // …but if there was already a literal, it's not implied
                                unit = None;
                                break;
                            }
                            None => unit = Some((*literal, self.id.unwrap())), // still, if everything so far is false, this literal must be true, for now…
                        }
                    }
                }
            }
        }
        unit
    }
}
