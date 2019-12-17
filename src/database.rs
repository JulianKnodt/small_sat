use crate::{clause::Clause, literal::Literal};
use std::{
  hash::{Hash, Hasher},
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

  // Learnt clauses from each solver and the clock # of the latest clause.
  // The clock # must be explicitly tracked since the database might be compacted.
  // .0 is num written
  // .1 is the actual data
  // .2 is the number deleted
  learnt_clauses: Vec<RwLock<(usize, Vec<Weak<Clause>>, usize)>>,

  /// A short circuited solution
  /// Is a nested option to indicate no solution found or
  /// there is no solution.
  pub(crate) solution: RwLock<Option<Option<Vec<bool>>>>,
}

impl ClauseDatabase {
  pub fn new(max_var: usize, mut initial_clauses: Vec<Clause>) -> Self {
    let learnt_clauses = vec![RwLock::new((0, vec![], 0))];
    // Can't trust these darned CNF files
    initial_clauses.sort_unstable();
    initial_clauses.dedup();
    Self {
      curr_id: RwLock::new(0),
      max_var,
      initial_clauses: initial_clauses.into_iter().map(Arc::new).collect(),
      learnt_clauses,
      solution: RwLock::new(None),
    }
  }
  /// Adds a solution to this database
  pub fn add_solution(&self, sol: Option<Vec<bool>>) {
    self.solution.write().unwrap().replace(sol);
  }
  pub fn get_solution(&self) -> Option<Option<Vec<bool>>> {
    self.solution.read().unwrap().as_ref().cloned()
  }
  /// adds a batch of learnt clauses to the database and returns the new timestamp of the
  /// process
  pub fn add_learnts(&self, id: usize, c: &mut Vec<ClauseRef>) -> usize {
    let mut learnt_clauses = self.learnt_clauses[id].write().unwrap();
    learnt_clauses.0 += c.len();
    learnt_clauses
      .1
      .extend(c.drain(..).map(|cref| Arc::downgrade(&cref.inner)));
    learnt_clauses.0
  }
  /// returns the number of solvers expected for this database
  pub fn num_solvers(&self) -> usize { self.learnt_clauses.len() }
  pub fn next_id(&self) -> usize {
    let mut id = self.curr_id.write().unwrap();
    *id += 1;
    *id - 1
  }

  pub fn iter(&self) -> impl Iterator<Item = ClauseRef> + '_ {
    let out = (0..self.initial_clauses.len()).map(move |i| ClauseRef {
      inner: self.initial_clauses[i].clone(),
    });
    let mut new = vec![];
    self.since(&mut new, &mut vec![0; self.num_solvers()]);
    out.chain(new.into_iter())
  }
  pub fn initial(&self) -> &Vec<Arc<Clause>> { &self.initial_clauses }
  /// Writes the new clauses into "into", and updates the timestamps.
  /// Returns the number of clauses written.
  pub fn since<T: Extend<ClauseRef>>(&self, into: &mut T, times: &mut Vec<usize>) {
    assert_eq!(self.learnt_clauses.len(), times.len());
    times.iter_mut().enumerate().for_each(|(i, written)| {
      match &self.learnt_clauses[i].try_read() {
        Err(_) => {},
        Ok(learnt_clauses) => {
          into.extend(
            learnt_clauses
              .1
              .iter()
              .skip(*written - learnt_clauses.2)
              .filter_map(Weak::upgrade)
              .map(|inner| ClauseRef { inner }),
          );
          *written = learnt_clauses.0;
        },
      }
    });
  }
  pub fn compact(&self, id: usize) {
    match self.learnt_clauses[id].try_write() {
      Err(_) => {},
      Ok(mut learnt) => {
        let original = learnt.1.len();
        learnt.1.retain(|weak| weak.strong_count() > 0);
        learnt.2 = learnt.1.len() - original;
      },
    };
  }
  pub fn resize_to(&mut self, n: usize) { self.learnt_clauses.resize_with(n, Default::default); }
}

#[derive(Debug, Clone, PartialOrd, Ord)]
pub struct ClauseRef {
  pub(crate) inner: Arc<Clause>,
}

impl PartialEq for ClauseRef {
  fn eq(&self, o: &Self) -> bool { Arc::ptr_eq(&self.inner, &o.inner) }
}
impl Eq for ClauseRef {}
impl Hash for ClauseRef {
  fn hash<H: Hasher>(&self, state: &mut H) { self.inner.hash(state); }
}

impl Deref for ClauseRef {
  type Target = Clause;
  fn deref(&self) -> &Self::Target { &*self.inner }
}

impl From<Clause> for ClauseRef {
  fn from(clause: Clause) -> Self {
    Self {
      inner: Arc::new(clause),
    }
  }
}

impl ClauseRef {
  pub fn locked(&self, lit: Literal, assns: &[Option<bool>], causes: &[Option<Self>]) -> bool {
    // check that this clause has this lit
    debug_assert!(self.literals.binary_search(&lit).is_ok());
    lit.assn(assns) == Some(true)
      && causes[lit.var()]
        .as_ref()
        .map_or(false, |reason| Arc::ptr_eq(&reason.inner, &self.inner))
  }
}
