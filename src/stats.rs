use std::time::{Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
  /// how many restarts did this solver perform
  restarts: u32,
  /// how many clauses did this solver learn
  clauses_learned: u32,
  /// how many propogations were there
  propogations: u32,
  /// how many clauses did this solver write to the database
  written_clauses: u32,
  /// how many clauses did this solver have transferred to it
  transferred_clauses: u32,

  /// The start time of this solver
  pub start_time: Instant,
}

#[derive(Debug, Clone, Copy)]
pub enum Record {
  Restart,
  LearnedClause,
  Propogation,
  Written(u32),
  Transferred(u32),
}

impl Stats {
  pub fn new() -> Self {
    Self {
      restarts: 0,
      clauses_learned: 0,
      propogations: 0,
      written_clauses: 0,
      transferred_clauses: 0,
      start_time: Instant::now(),
    }
  }
  pub fn record(&mut self, rec: Record) {
    match rec {
      Record::Restart => self.restarts += 1,
      Record::LearnedClause => self.clauses_learned += 1,
      Record::Propogation => self.propogations += 1,
      Record::Written(n) => self.propogations += n,
      Record::Transferred(n) => self.propogations += n,
    };
  }
}
