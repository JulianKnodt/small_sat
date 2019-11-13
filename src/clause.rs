use crate::literal::Literal;
use std::fmt;

/*
pub struct ClauseHeader {
  mark: u8,
  learnt: bool,
}
*/

/// A CNF clause, where each of the literals is some variable in the entire expression
#[derive(Clone, Debug, PartialEq, Hash)]
pub struct Clause {
  pub(crate) literals: Vec<Literal>,
  //  marked_for_deletion: bool,
  learnt: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClauseState<'a> {
  SAT,
  UNSAT,
  UNDETERMINED,
  // Unit specifies which literal in this clause is currently unit
  UNIT(&'a Literal),
}

impl<'a> ClauseState<'a> {
  pub fn unit_assn(&self) -> Option<(usize, bool)> {
    if let ClauseState::UNIT(l) = self {
      Some((l.var(), l.true_eval()))
    } else {
      None
    }
  }
}

impl Clause {
  pub fn push(&mut self, lit: Literal) { self.literals.push(lit); }
  pub fn is_empty(&self) -> bool { self.literals.is_empty() }
  pub fn is_sat(&self, assns: &Vec<Option<bool>>) -> bool {
    self
      .literals
      .iter()
      .any(|lit| lit.assn(assns).unwrap_or(false))
  }
  pub fn unassigned_literals(&self, assns: &Vec<Option<bool>>) -> Vec<&Literal> {
    self
      .literals
      .iter()
      .filter(|lits| lits.assn(assns).is_none())
      .collect()
  }
  pub fn state<'a>(&'a self, assignments: &Vec<Option<bool>>) -> ClauseState<'a> {
    if self.is_empty() {
      return ClauseState::UNSAT;
    }
    let mut recent_unassigned = None;
    let curr = self
      .literals
      .iter()
      .map(|lit| (lit, lit.assn(assignments)))
      .fold(
        (0usize, 0usize, 0usize),
        |(unassigned, t, f), (lit, n)| match n {
          None => {
            recent_unassigned.replace(lit);
            (unassigned + 1, t, f)
          },
          Some(true) => (unassigned, t + 1, f),
          Some(false) => (unassigned, t, f + 1),
        },
      );
    match curr {
      (_, t, _) if t > 0 => ClauseState::SAT,
      (u, _, _) if u > 1 => ClauseState::UNDETERMINED,
      (1, _, _) => ClauseState::UNIT(recent_unassigned.unwrap()),
      (0, 0, _) => ClauseState::UNSAT,
      (_, _, _) => unreachable!(),
    }
  }
  // gets the assignment for a unit clause
  pub fn unit_assignment(&self, assignments: &Vec<Option<bool>>) -> Option<(usize, bool)> {
    let unassigned = self
      .literals
      .iter()
      .filter_map(|lit| match lit.assn(assignments) {
        Some(_) => None,
        None => Some(lit),
      })
      .collect::<Vec<_>>();
    if unassigned.len() != 1 {
      return None;
    }
    Some((unassigned[0].var(), unassigned[0].true_eval()))
  }
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
