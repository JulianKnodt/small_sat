use crate::{
  clause::{Clause},
};

#[derive(Clone, Debug, PartialEq)]
pub struct WatchList<'a> {
  watched: Vec<Vec<&'a Clause>>,
}

impl WatchList {
  // new creates a new watch list, assuming that none of the clauses are satisfied
  fn new(clauses: &'a Vec<Clause>, max_vars: usize) -> Self<'a> {
    let mut watched = Vec::with_capacity(max_vars);
    clauses.iter().for_each(|clause| {
      watched.get(clause.literals()[0].var())
    });
    WatchList{watched}
  }
}
