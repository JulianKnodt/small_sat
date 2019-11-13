use crate::clause::Clause;
use std::{
  ops::Deref,
  sync::{Arc, RwLock},
};

// maybe want some append only log?

#[derive(Debug)]
pub struct ClauseDatabase {
  // the max number of variables in this set of clauses
  max_vars: usize,

  // initial set of read only clauses.
  // They should only be simplified to be equivalent to
  // the original set of clauses given.
  pub(crate) initial_clauses: Vec<Clause>,

  // any learnt clause, each one is likely to be added individually so it is more efficient to
  // store them each individually

  // TODO isolate this behind some nice APIs? Hard given the lock
  pub(crate) learnt_clauses: RwLock<Vec<Arc<Clause>>>,
}

impl ClauseDatabase {
  pub fn add_learnt(&self, c: Clause) {
    self
      .learnt_clauses
      .write()
      .expect("Failed to get clauses in add_clause")
      .push(Arc::new(c));
  }
  pub fn borrow_clause<'a>(&'a self, cref: &'a ClauseRef) -> &'a Clause {
    match cref {
      ClauseRef::Initial(i) => &self.initial_clauses[*i],
      ClauseRef::Learnt(arc) => arc.deref(),
    }
  }
  // potentially expensive as it clones all the references to the learnt clauses at the same
  // time
  pub fn iter(&self) -> impl Iterator<Item = ClauseRef> {
    (0..self.initial_clauses.len())
      .map(|idx| ClauseRef::Initial(idx))
      .chain(
        self
          .learnt_clauses
          .read()
          .unwrap()
          .iter()
          .map(|r| ClauseRef::Learnt(Arc::clone(r)))
          .collect::<Vec<_>>()
          .into_iter(),
      )
  }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClauseRef {
  // Learnt clauses are just atomically referenced pointers
  Learnt(Arc<Clause>),
  // Since initial is readonly, it's safe to store a usize
  Initial(usize),
}

impl From<Vec<Clause>> for ClauseDatabase {
  fn from(v: Vec<Clause>) -> Self {
    Self {
      initial_clauses: v,
      learnt_clauses: RwLock::new(vec![]),
    }
  }
}
