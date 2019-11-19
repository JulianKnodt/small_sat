use crate::clause::Clause;
use std::{
  ops::Deref,
  sync::{Arc, RwLock, Weak},
};

// maybe want some append only log?

#[derive(Debug)]
pub struct ClauseDatabase {
  // the max number of variables in this set of clauses
  pub(crate) max_vars: usize,

  // initial set of read only clauses.
  // They should only be simplified to be equivalent to
  // the original set of clauses given.
  pub initial_clauses: Vec<Clause>,

  // any learnt clause, each one is likely to be added individually so it is more efficient to
  // store them each individually

  // TODO isolate this behind some nice APIs? Hard given the lock
  pub(crate) learnt_clauses: RwLock<Vec<Weak<Clause>>>,
}

impl ClauseDatabase {
  pub fn add_learnt(&self, c: Weak<Clause>) {
    self
      .learnt_clauses
      .write()
      .expect("Failed to get clauses in add_clause")
      .push(c);
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
      .chain(self.since(0))
  }
  pub fn since(&self, time: usize) -> impl Iterator<Item = ClauseRef> {
    self
      .learnt_clauses
      .read()
      .unwrap()
      .iter()
      .skip(time)
      .filter_map(Weak::upgrade)
      .map(|r| ClauseRef::Learnt(r))
      .collect::<Vec<_>>()
      .into_iter()
  }
}

impl From<Vec<Clause>> for ClauseDatabase {
  fn from(v: Vec<Clause>) -> Self {
    let max_vars = v.iter().map(|c| c.max_var()).max().unwrap_or(0) + 1;
    Self {
      max_vars,
      initial_clauses: v,
      learnt_clauses: RwLock::new(vec![]),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClauseRef {
  // Learnt clauses are just atomically referenced pointers
  Learnt(Arc<Clause>),
  // Since initial is readonly, it's safe to store a usize
  Initial(usize),
}

impl From<Arc<Clause>> for ClauseRef {
  fn from(clause: Arc<Clause>) -> Self { ClauseRef::Learnt(clause) }
}
