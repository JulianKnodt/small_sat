use crate::clause::Clause;

// TODO this doesn't work because it orders by key not value.
#[derive(Debug, PartialEq)]
pub struct VariableStateDecayCache {
  // (var, occurrence count)
  occurrences: Vec<(usize, f32)>,
  // rate of decay for this state decay machine
  decay_rate: f32,
}

pub const SAMPLE_DECAY_RATE: f32 = 1.2;

impl VariableStateDecayCache {
  /// creates a new variable state decay cache with the given number of vars, and a decay rate
  pub fn new(decay_rate: f32) -> Self {
    Self {
      occurrences: vec![],
      decay_rate: decay_rate,
    }
  }
  /// decays the current occurrence account
  pub fn decay(&mut self) {
    let decay_rate = self.decay_rate;
    self
      .occurrences
      .iter_mut()
      .for_each(|(_, v)| *v /= decay_rate);
  }
  /// adds a set of clauses to this cache
  pub fn with_clauses(&mut self, clauses: &Vec<Clause>) {
    clauses.iter().for_each(|c| self.add_clause(c))
  }
  /// Can be used for adding learnt clauses
  pub fn add_clause(&mut self, c: &Clause) {
    c.literals().iter().for_each(|lit| {
      let found = self
        .occurrences
        .iter_mut()
        .find(|&&mut (var, _)| var == lit.var)
        .map(|cnt| cnt.1 += 1.0);
      if found.is_none() {
        self.occurrences.push((lit.var, 1.0));
      }
    });
    self
      .occurrences
      .sort_unstable_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap());
  }
  pub fn highest_items(&self) -> std::slice::Iter<'_, (usize, f32)> {
    // Return in order of highest -> lowest
    self.occurrences.iter()
  }
}
