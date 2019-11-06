use crate::{clause::Clause, literal::Literal};

use std::collections::HashMap;

/*
An implementation of occurrence lists based on MiniSat's OccList
*/

pub struct WatchList {
  // literal -> Vec<idx of clause, other literal being stored>
  occurrences: Vec<HashMap<usize, Option<u32>>>,
}

/// leaves enough space for both true and false variables up to max_vars.
fn space_for_max_vars(size: usize) -> usize { (size << 1) + 1 }

fn watch(occs: &mut Vec<HashMap<usize, Option<u32>>>, idx: usize, clause_db: &Vec<Clause>) -> bool {
  let clause = &clause_db[idx];
  let two_lits = clause.literals.iter().take(2).collect::<Vec<_>>();
  match two_lits.len() {
    0 => return false,
    1 => occs[(!two_lits[0]).raw() as usize]
      .insert(idx, None)
      .is_none(),
    2 =>
      occs[(!two_lits[0]).raw() as usize]
        .insert(idx, Some(1))
        .is_none()
        && occs[(!two_lits[1]).raw() as usize]
          .insert(idx, Some(0))
          .is_none(),
    _ => unreachable!(),
  }
}

impl From<(&Vec<Clause>, usize)> for WatchList {
  fn from(sat_desc: (&Vec<Clause>, usize)) -> Self {
    let (clause_db, max_vars) = sat_desc;
    let mut occurrences = vec![HashMap::new(); space_for_max_vars(max_vars)];
    let all_ok = (0..clause_db.len()).all(|idx| watch(&mut occurrences, idx, clause_db));
    assert!(
      all_ok,
      "Failed with initial add into watch list, bug in implementation"
    );
    WatchList { occurrences }
  }
}

// TODO modify so that when set_false is run or set_true is run the other set gets deleted.

impl WatchList {
  /// notifies the watch list that a literal was set to false
  /// and updates the watch list to point to the next set of watched literals.
  /// will also return a set of unit literals
  fn set_false(
    &mut self,
    false_lit: Literal,
    assns: &Vec<Option<bool>>,
    clause_db: &Vec<Clause>,
  ) -> Option<Result<Vec<usize>, usize>> {
    // Check caller actually assigned the var to false before calling this
    assert_eq!(assns[false_lit.var()], Some(false));

    let mut todo = vec![];
    let output = self
      .occurrences
      .get_mut(false_lit.raw() as usize)
      .map(|clauses| {
        clauses
          .drain()
          .filter_map(|(clause, other_lit)| match other_lit {
            None => Some(Err(clause)),
            Some(watched_lit) => {
              let next = clause_db[clause]
                .literals
                .iter()
                .find(|lit| assns[lit.var()].is_none())
                .map(|next_lit| (!next_lit).raw());
              todo.push((watched_lit, clause, next));
              // If there is no next item, then this is a unit clause
              // otherwise it's not
              match next {
                None => Some(Ok(clause)),
                Some(_) => None,
              }
            },
          })
          .collect()
      });
    for (watched_lit, clause, next) in todo {
      let out = self.occurrences[watched_lit as usize].insert(clause, next);
      assert_ne!(out, None);
      if let Some(other_watch) = next {
        self.occurrences[other_watch as usize].insert(clause, Some(watched_lit));
      }
    }
    output
  }
  // marks a literal as true, removing it from the set of watched clauses
  fn set_true(&mut self, true_lit: Literal) {
    self
      .occurrences
      .get_mut(true_lit.raw() as usize)
      .map(|clauses| {
        clauses
          .drain()
          .filter_map(|(clause, other)| other.map(|other_lit| (clause, other_lit)))
          .collect::<Vec<_>>()
      })
      .map(|to_rm| {
        to_rm.into_iter().for_each(|(clause, other)| {
          assert_ne!(self.occurrences[other as usize].remove(&clause), None)
        });
      });
  }
  /// Adds the clauses from starting index onwards.
  /// returns true if successful false if not
  pub fn add_clauses(&mut self, starting_idx: usize, clause_db: &Vec<Clause>) -> bool {
    (starting_idx..clause_db.len()).all(|idx| watch(&mut self.occurrences, idx, clause_db))
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::dimacs::from_dimacs;
  fn example_clauses_and_vars() -> (Vec<Clause>, usize) {
    let path =
      "/Users/julianknodt/Desktop/programming/projects/rustjects/small_sat/src/bin/data/sample.cnf";
    from_dimacs(path).expect("Failed to open sample.cnf")
  }
  #[test]
  fn test_watchlist() {
    let (clauses, max_vars) = example_clauses_and_vars();
    let wl = WatchList::from((&clauses, max_vars));
  }
}
