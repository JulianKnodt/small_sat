use crate::{clause::Clause, literal::Literal};
use std::{
  ops::Deref,
  hash::{Hash, Hasher},
  sync::{Arc, RwLock, Weak, atomic::{AtomicU64, Ordering}},
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

  // TODO track median activity usage here?

  pub(crate) solution: RwLock<Option<Vec<bool>>>,
}

impl ClauseDatabase {
  pub fn new(max_var: usize, mut initial_clauses: Vec<Clause>) -> Self {
    let learnt_clauses = vec![RwLock::new(vec![])];
    // Can't trust these darned CNF files
    initial_clauses.sort_unstable();
    initial_clauses.dedup();
    Self {
      curr_id: RwLock::new(0),
      max_var,
      initial_clauses: initial_clauses.into_iter().map(|it| Arc::new(it)).collect(),
      learnt_clauses,
      solution: RwLock::new(None),
    }
  }
  /// Adds a solution to this database
  pub fn add_solution(&self, sol: Vec<bool>) { self.solution.write().unwrap().replace(sol); }
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

#[derive(Debug, Clone)]
pub struct ClauseRef {
  pub(crate) inner: Arc<Clause>,
  pub(crate) activity: Arc<AtomicU64>,
}

impl Deref for ClauseRef {
  type Target = Clause;
  fn deref(&self) -> &Self::Target { &*self.inner }
}

impl From<Arc<Clause>> for ClauseRef {
  fn from(clause: Arc<Clause>) -> Self {
    Self {
      inner: clause,
      // Everything starts with an activity of one when created
      activity: Arc::new(AtomicU64::new(1)),
    }
  }
}

impl PartialEq for ClauseRef {
  fn eq(&self, o: &Self) -> bool {
    self.inner == o.inner
  }
}
impl Eq for ClauseRef {}

impl Hash for ClauseRef {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.inner.hash(state)
  }
}

impl ClauseRef {
  pub fn locked(
    &self,
    lit: Literal,
    assns: &Vec<Option<bool>>,
    causes: &Vec<Option<Self>>,
  ) -> bool {
    // check that this clause has this lit
    assert!(self.literals.binary_search(&lit).is_ok());
    lit.assn(assns) == Some(true)
      && causes[lit.var()]
        .as_ref()
        .map_or(false, |reason| Arc::ptr_eq(&reason.inner, &self.inner))
  }
  pub fn boost(&self) {
    self.activity.fetch_add(1, Ordering::Relaxed);
  }
  pub fn curr_activity(&self) -> u64 { self.activity.load(Ordering::Relaxed) }
}
