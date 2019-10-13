use crate::literal::Literal;
use std::{collections::HashSet, fmt};

/// A CNF clause, where each of the literals is some variable in the entire expression
#[derive(Clone, Debug, PartialEq)]
pub struct Clause {
  literals: Vec<Literal>,
  learnt: bool,
}

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClauseState<'a> {
  SAT,
  UNSAT,
  UNDETERMINED,
  // Unit specifies which literal in this clause is currently unit
  UNIT(&'a Literal),
}

impl Clause {
  pub fn new() -> Self {
    Clause {
      literals: vec![],
      learnt: false,
    }
  }
  pub fn literals(&self) -> &Vec<Literal> { &self.literals }
  pub fn push(&mut self, lit: Literal) { self.literals.push(lit); }
  pub fn is_empty(&self) -> bool { self.literals.is_empty() }
  pub fn is_sat(&self, assns: &Vec<Option<bool>>) -> bool {
    self.literals.iter().any(|lit| lit.assn(assns).unwrap_or(false))
  }
  pub fn subsumes(&self, o: &Self) -> bool {
    self
      .literals
      .iter()
      .collect::<HashSet<_>>()
      .is_superset(&o.literals.iter().collect::<HashSet<_>>())
  }
  pub fn state<'a>(&'a self, assignments: &Vec<Option<bool>>) -> ClauseState<'a> {
    if self.is_empty() {
      return ClauseState::UNSAT;
    }
    let curr = self.literals.iter().map(|lit| lit.assn(assignments)).fold(
      (0usize, 0usize, 0usize),
      |(unassigned, t, f), n| match n {
        None => (unassigned + 1, t, f),
        Some(true) => (unassigned, t + 1, f),
        Some(false) => (unassigned, t, f + 1),
      },
    );
    match curr {
      (_, t, _) if t > 0 => ClauseState::SAT,
      (u, _, _) if u > 1 => ClauseState::UNDETERMINED,
      (1, _, _) => ClauseState::UNIT(self.get_unit_lit(assignments)),
      (0, 0, _) => ClauseState::UNSAT,
      (_, _, _) => unreachable!(),
    }
  }
  fn get_unit_lit<'a>(&'a self, assignments: &Vec<Option<bool>>) -> &'a Literal {
    self
      .literals
      .iter()
      .find(|lit| lit.assn(assignments).is_none())
      .unwrap()
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
    Some((unassigned[0].var, unassigned[0].true_eval()))
  }
}

impl From<Vec<Literal>> for Clause {
  fn from(lits: Vec<Literal>) -> Self {
    Clause {
      literals: lits,
      learnt: false,
    }
  }
}
