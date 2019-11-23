#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stats {
  restarts: u32,
  clauses_learned: u32,
  propogations: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Record {
  Restart,
  LearnedClause,
  Propogation,
}

impl Stats {
  pub fn record(&mut self, rec: Record) {
    match rec {
      Record::Restart => self.restarts += 1,
      Record::LearnedClause => self.clauses_learned += 1,
      Record::Propogation => self.propogations += 1,
    };
  }
}
