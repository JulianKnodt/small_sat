use crate::clause::{Clause};

#[derive(Clone, Debug)]
pub struct Conflict {
  level: usize,
  causes: Vec<usize>,
}

#[derive(Debug)]
pub struct Implication {
  // indeces into some global implication list about
  literal: usize,
  assigned_to: bool,
  level: usize,
  // causes will only be set if implied due to BCP
  causes: Option<Vec<usize>>,
}

impl Implication {
  fn decision(literal: usize, assn: bool, level: usize) -> Self {
    Self{
      literal: literal,
      assigned_to: assn,
      level: level,
      causes: None,
    }
  }
  fn implication(literal: usize, assn: bool, level: usize, clause: &Clause) -> Self {
    unimplemented!();
  }
}

pub fn conflict_clause(conflict: &Conflict, Vec<Implication>) -> Clause {
  let mut choices = vec!(conflict.causes.clone());
  // TODO backtrack from conflict node
  // see if can reach determination that caused this

  // Which literals are to be blamed
  let mut conflicting_literals = vec!();
  while let Some(l) = choices.last() {
    unimplemented!();
  }
}
