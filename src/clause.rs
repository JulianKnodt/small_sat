use crate::literal::Literal;
use std::{
  fmt,
  hash::{Hash, Hasher},
  sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
  },
};

/// A CNF clause, where each of the literals is some variable in the entire expression
#[derive(Debug)]
pub struct Clause {
  /// Literals for this clause
  pub(crate) literals: Vec<Literal>,
  /// True iff this clause was from the initial set of clauses
  pub(crate) initial: bool,
  /// Clause activity, used for compaction
  pub(crate) activity: Arc<AtomicU64>,
}

impl PartialEq for Clause {
  fn eq(&self, o: &Self) -> bool { self.literals == o.literals }
}
impl Eq for Clause {}
impl PartialOrd for Clause {
  fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
    self.literals.partial_cmp(&o.literals)
  }
}
impl Ord for Clause {
  fn cmp(&self, o: &Self) -> std::cmp::Ordering { self.partial_cmp(&o).unwrap() }
}

impl Hash for Clause {
  fn hash<H: Hasher>(&self, state: &mut H) { self.literals.hash(state) }
}

impl Clause {
  /// returns true if this clause has no literals.
  pub fn is_empty(&self) -> bool { self.literals.is_empty() }
  /// Returns true if this clause contains both a literal and its negation.
  pub fn is_tautology(&self) -> bool {
    let mut seen: Vec<&Literal> = Vec::with_capacity(self.literals.len());
    self.literals.iter().any(|lit| {
      if seen.iter().any(|prev| prev.is_negation(lit)) {
        return true;
      };
      seen.push(lit);
      false
    })
  }
  /// returns true if any literal is true based on the assignment vector
  pub fn is_sat(&self, final_assns: &Vec<bool>) -> bool {
    self
      .literals
      .iter()
      .any(|lit| final_assns[lit.var()] ^ lit.negated())
  }
  /// Increases the ordering of this clause
  pub fn boost(&self) { self.activity.fetch_add(1, Ordering::SeqCst); }
  /// SeqCst Atomic load of the activity for this clause
  pub fn curr_activity(&self) -> u64 { self.activity.load(Ordering::SeqCst) }
}

impl From<Vec<Literal>> for Clause {
  fn from(mut lits: Vec<Literal>) -> Self {
    // is this necessary? maybe we can lazily handle this elsewhere?
    lits.sort_unstable();
    lits.dedup();
    Self {
      literals: lits,
      initial: false,
      activity: Arc::new(AtomicU64::new(0)),
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;
  fn example_clause() -> Clause {
    Clause::from(vec![Literal::from(-1), Literal::from(2), Literal::from(-3)])
  }
  fn tautology() -> Clause { Clause::from(vec![Literal::from(-1), Literal::from(1)]) }
  #[test]
  fn check_tautology() {
    assert!(tautology().is_tautology());
    assert!(!example_clause().is_tautology());
  }
}

/// Shows disjuncted literals with negations
impl fmt::Display for Clause {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "(")?;
    if self.literals.len() > 0 {
      write!(f, "{}", self.literals[0])?;
      for lit in self.literals.iter().skip(1) {
        write!(f, " | {}", lit)?;
      }
    }
    write!(f, ")")
  }
}
