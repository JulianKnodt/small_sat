use crate::clause::Clause;
use std::{
  ops::Deref,
  sync::{Arc, RwLock, Weak},
};

// maybe want some append only log?

#[derive(Debug)]
pub struct ClauseDatabase {
  // the max number of variables in this set of clauses
  pub(crate) max_var: usize,
  curr_id: RwLock<usize>,

  // initial set of read only clauses.
  // They should only be simplified to be equivalent to
  // the original set of clauses given.
  pub initial_clauses: Vec<Arc<Clause>>,

  // any learnt clause, each one is likely to be added individually so it is more efficient to
  // store them each individually

  // TODO isolate this behind some nice APIs? Hard given the lock
  pub(crate) learnt_clauses: Vec<RwLock<Vec<Weak<Clause>>>>,
}

impl ClauseDatabase {
  pub fn new(max_var: usize, initial_clauses: Vec<Clause>) -> Self {
    let learnt_clauses = vec![RwLock::new(vec![])];
    Self {
      curr_id: RwLock::new(0),
      max_var,
      initial_clauses: initial_clauses.into_iter().map(|it| Arc::new(it)).collect(),
      learnt_clauses,
    }
  }
  /// adds a batch of learnt clauses to the database and returns the new timestamp of the
  /// process
  pub fn add_learnts(&self, id: usize, c: &mut Vec<Weak<Clause>>) -> usize {
    let mut learnt_clauses = self.learnt_clauses[id].write().unwrap();
    learnt_clauses.append(c);
    learnt_clauses.len()
  }
  /// returns the number of solvers expected for this database
  pub fn num_solvers(&self) -> usize { self.learnt_clauses.len() }
  pub fn next_id(&self) -> usize {
    let mut id = self.curr_id.write().unwrap();
    *id += 1;
    *id - 1
  }

  pub fn iter(&self) -> impl Iterator<Item = ClauseRef> + '_ {
    (0..self.initial_clauses.len())
      .map(move |i| ClauseRef::from(self.initial_clauses[i].clone()))
      .chain(
        self
          .since(&vec![0; self.learnt_clauses.len()])
          .0
          .into_iter(),
      )
  }
  pub fn initial(&self) -> &Vec<Arc<Clause>> { &self.initial_clauses }
  pub fn since(&self, times: &Vec<usize>) -> (Vec<ClauseRef>, Vec<usize>) {
    assert_eq!(self.learnt_clauses.len(), times.len());
    let mut out = vec![];
    let new_timestamps = (0..times.len())
      .map(|i| {
        let learnt_clauses = &self.learnt_clauses[i].read().unwrap();
        out.extend(
          learnt_clauses
            .iter()
            .skip(times[i])
            .filter_map(Weak::upgrade)
            .map(|r| ClauseRef::from(r)),
        );
        learnt_clauses.len()
      })
      .collect::<Vec<_>>();
    (out, new_timestamps)
  }
  pub fn resize_to(&mut self, n: usize) { self.learnt_clauses.resize_with(n, Default::default); }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClauseRef {
  pub(crate) inner: Arc<Clause>,
}

impl Deref for ClauseRef {
  type Target = Clause;
  fn deref(&self) -> &Self::Target { &*self.inner }
}

impl From<Arc<Clause>> for ClauseRef {
  fn from(clause: Arc<Clause>) -> Self { ClauseRef { inner: clause } }
}
