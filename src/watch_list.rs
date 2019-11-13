use crate::{
  database::{ClauseDatabase, ClauseRef},
  literal::Literal,
};

use std::collections::HashMap;

/*
An implementation of occurrence lists based on MiniSat's OccList
*/

#[derive(Debug, PartialEq)]
pub struct WatchList {
  // raw literal ->  Vec(OtherLiteral, which Clause is being watched)
  occurrences: Vec<Vec<(Literal, ClauseRef)>>,

  // separate buffer for unit clauses
  units: HashMap<Literal, ClauseRef>,

  // List of completed clauses
  // completed: Vec<ClauseRef>
}

/// leaves enough space for both true and false variables up to max_vars.
fn space_for_all_lits(size: usize) -> usize { (size << 1) + 1 }

// only to be called for initial clauses
fn watch(
  occs: &mut Vec<Vec<(Literal, ClauseRef)>>,
  cref: &ClauseRef,
  db: &ClauseDatabase,
) -> Option<Literal> {
  let lits = db
    .borrow_clause(cref)
    .literals
    .iter()
    .take(2)
    .collect::<Vec<_>>();
  match lits.len() {
    0 => panic!("Empty clause passed to watch: {:?}", cref),
    1 => return Some(lits[0]),
    2 => {
      occs[lits[0].raw() as usize].push((*lits[1], cref.clone()));
      occs[lits[1].raw() as usize].push((*lits[0], cref.clone()));
    },
    _ => unreachable!(),
  }
  None
}

impl From<&ClauseDatabase> for WatchList {
  fn from(db: &ClauseDatabase) -> Self {
    let mut occurrences = vec![vec![]; space_for_all_lits(db.max_vars)];
    let units = db
      .iter()
      .filter_map(|cref| watch(&mut occurrences, cref, db).map(|lit| (lit, cref)))
      .collect();
    // occurrences.iter_mut().for_each(|occ| occ.sort_by_key(|other_ref| other_ref.0))
    WatchList { occurrences, units }
  }
}

impl WatchList {
  pub fn set(&mut self, lit: Literal, assns: &Vec<Option<bool>>, clause_db: &ClauseDatabase) {
    // Sanity check that we actually assigned this variable
    assert_eq!(assns[lit.var()], Some(lit.val()));
    self.set_true(lit);
    self.set_false(!lit, assns, clause_db);
  }
  /// Notify this watchlist that a literal was unset in backtracking
  pub fn unset(&mut self, _lit: Literal) { unimplemented!() }
  fn set_false(&mut self, lit: Literal, assns: &Vec<Option<bool>>, clause_db: &ClauseDatabase) {
    // which references do we need to update
    let mut todo : Vec<(Literal, &ClauseRef, Literal)> = vec![];
    let new_units

    if let Some(clauses) = self.occurrences.get_mut(lit.raw() as usize) {
      clauses.drain().for_each(|(other_lit, cref)| {
        let next = clause_db
          .borrow_clause(cref)
          .literals
          .iter()
          .filter(|&&lit| lit != other_lit)
          .find(|lit| assns[lit.var()].is_none());
        match next {
          None => self.units.push(,
          Some(next) => todo.push((other_lit, cref, next)),
        }
      });
    }
    for (watched_lit, clause, next) in todo {
      self.occurrences[watched_lit.raw()].iter_mut().find(|
        self.occurrences[watched_lit.raw() as usize].insert(clause, *next),
      self.occurrences[next.raw() as usize].insert(clause, watched_lit);
    }
  }
  // marks a literal as true, removing it from the set of watched clauses
  fn set_true(&mut self, lit: Literal) {
    self
      .occurrences
      .get_mut(lit.raw() as usize)
      .map(|clauses| clauses.drain().collect::<Vec<_>>())
      .map(|to_rm| {
        println!("to rm {:?}", to_rm);
        to_rm.into_iter().for_each(|(clause, other)| {
          assert_ne!(self.occurrences[other.raw() as usize].remove(&clause), None)
        });
      });
  }
  /*
  pub fn add_clauses(&mut self, starting_idx: usize, clause_db: ClauseDatabase) -> {
    (starting_idx..clause_db.len()).all(|idx| watch(&mut self.occurrences, idx, clause_db));
  }
  */
}

/*
#[cfg(test)]
mod test {
  use super::*;
  use crate::dimacs::from_dimacs;
  use std::path::Path;
  fn example_clauses_and_vars() -> (Vec<Clause>, usize) {
    let path = Path::new(file!());
    from_dimacs(path.parent().unwrap().join("bin/data/all_true.cnf"))
      .expect("Failed to open sample cnf file")
  }
  #[test]
  fn test_watchlist() {
    let (clauses, max_vars) = example_clauses_and_vars();
    let mut wl = WatchList::from((&clauses, max_vars));
    let mut assignments = vec![None; max_vars];
    let lit = Literal::new(0, true);
    println!("{}", lit);
    assignments[lit.var()] = Some(lit.val());
    wl.set(lit, &assignments, &clauses);
    panic!("{:?}", wl);
  }
}
*/
