use crate::literal::Literal;
use std::fmt;

/*
pub struct ClauseHeader {
  mark: u8,
  learnt: bool,
}
*/

/// A CNF clause, where each of the literals is some variable in the entire expression
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Clause {
  pub(crate) literals: Vec<Literal>,
  //  marked_for_deletion: bool,
  learnt: bool,
}

impl Clause {
  pub fn push(&mut self, lit: Literal) { self.literals.push(lit); }
  pub fn is_empty(&self) -> bool { self.literals.is_empty() }
  pub fn max_var(&self) -> usize { self.literals.iter().map(|lit| lit.var()).max().unwrap_or(0) }
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
  pub fn from_negated_lits(lits: Vec<Literal>) -> Self {
    Clause {
      literals: lits.into_iter().map(|lit| !lit).collect(),
      learnt: false,
    }
  }
  pub fn mark_learnt(&mut self) { self.learnt = true; }
}

impl From<Vec<Literal>> for Clause {
  fn from(mut lits: Vec<Literal>) -> Self {
    lits.sort_unstable();
    lits.dedup();
    Clause {
      literals: lits,
      learnt: false,
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;
  fn example_clause() -> Clause {
    Clause::from(vec![Literal::from(-1), Literal::from(2), Literal::from(-3)])
  }
  fn example_unit_assn() -> Vec<Option<bool>> { vec![Some(true), Some(false), None] }
  fn tautology() -> Clause { Clause::from(vec![Literal::from(-1), Literal::from(1)]) }
  #[test]
  fn check_tautology() {
    assert!(tautology().is_tautology());
    assert!(!example_clause().is_tautology());
  }
  #[test]
  fn unit_variable() {
    let ex = example_clause();
    assert!(ex.state(&example_unit_assn()) == ClauseState::UNIT(&ex.literals[2]));
    assert_eq!(ex.unit_assignment(&example_unit_assn()), Some((2, false)));
  }
}

/*
Display Implementation for Clauses
Shows disjuncted literals with negations
*/
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
