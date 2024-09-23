use crate::structures::{ClauseId, ClauseSource, LevelIndex};

pub type VariableId = usize;
use std::cell::Cell;

#[derive(Clone, Debug)]
pub struct Variable {
    name: String,
    decision_level: Option<LevelIndex>,
    id: VariableId,
    positive_occurrences: Vec<ClauseId>,
    negative_occurrences: Vec<ClauseId>,
    activity: Cell<f32>,
}

impl Variable {
    pub fn new(name: &str, id: VariableId) -> Self {
        Variable {
            name: name.to_string(),
            decision_level: None,
            id,
            positive_occurrences: Vec::new(),
            negative_occurrences: Vec::new(),
            activity: 0.0.into(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn decision_level(&self) -> Option<LevelIndex> {
        self.decision_level
    }

    pub fn clear_decision_level(&mut self) {
        self.decision_level = None
    }

    pub fn set_decision_level(&mut self, level: LevelIndex) {
        self.decision_level = Some(level)
    }

    pub fn id(&self) -> VariableId {
        self.id
    }

    pub fn increase_activity(&self, by: f32) {
        let mut activity = self.activity.get();
        activity += by;
        self.activity.replace(activity);
    }

    pub fn divide_activity(&self, by: f32) {
        let mut activity = self.activity.get();
        activity /= by;
        self.activity.replace(activity);
    }

    pub fn activity(&self) -> f32 {
        self.activity.get()
}

    pub fn note_occurence(&mut self, clause_id: ClauseId, source: ClauseSource, polarity: bool) {
        if let ClauseSource::Resolution = source {
            self.increase_activity(1.0)
        }

        match polarity {
            true => self.positive_occurrences.push(clause_id),
            false => self.negative_occurrences.push(clause_id),
        }
    }

    pub fn occurrences(&self) -> impl Iterator<Item = ClauseId> + '_ {
        self.positive_occurrences
            .iter()
            .chain(&self.negative_occurrences)
            .cloned()
    }
}

impl PartialOrd for Variable {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Variable {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name.cmp(&other.name)
    }
}

impl PartialEq for Variable {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Variable {}
