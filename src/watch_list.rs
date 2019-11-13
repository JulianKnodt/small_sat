use crate::{
  database::{ClauseDatabase, ClauseRef},
  literal::Literal,
};

use std::collections::HashMap;

/*
An implementation of occurrence lists based on MiniSat's OccList
*/

#[derive(Clone, Debug, PartialEq)]
pub struct WatchList {
  // raw literal ->  Vec(OtherLiteral, which Clause is being watched)
  occurrences: Vec<HashMap<ClauseRef, Literal>>,
  // TODO Convert to HashMap, somehow thought clause ref didn't implement hash

  // separate buffer for unit clauses
  pub(crate) units: HashMap<Literal, ClauseRef>,
  // List of completed clauses
  // completed: Vec<ClauseRef>
}

/// leaves enough space for both true and false variables up to max_vars.
fn space_for_all_lits(size: usize) -> usize { (size << 1) + 1 }

// adds some clause from a database to this watch list
fn watch(
  occs: &mut Vec<HashMap<ClauseRef, Literal>>,
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
    1 => return Some(*lits[0]),
    2 => {
      occs[lits[0].raw() as usize].insert(cref.clone(), *lits[1]);
      occs[lits[1].raw() as usize].insert(cref.clone(), *lits[0]);
    },
    _ => unreachable!(),
  }
  None
}

// removes some literal and a clause_ref from an occ list
fn remove(occs: &mut Vec<HashMap<ClauseRef, Literal>>, lit: Literal, cref: &ClauseRef) {
  let watched = &mut occs[lit.raw() as usize];
  assert_ne!(watched.remove(cref), None);
}

impl From<&ClauseDatabase> for WatchList {
  fn from(db: &ClauseDatabase) -> Self {
    let mut occurrences = vec![HashMap::new(); space_for_all_lits(db.max_vars)];
    let units = db
      .iter()
      .filter_map(|cref| watch(&mut occurrences, &cref, db).map(|lit| (lit, cref)))
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
    let mut todo: Vec<(Literal, ClauseRef, Literal)> = vec![];
    let mut units = vec![];

    if let Some(clauses) = self.occurrences.get_mut(lit.raw() as usize) {
      clauses.drain().for_each(|(cref, other_lit)| {
        let next = clause_db
          .borrow_clause(&cref)
          .literals
          .iter()
          .filter(|&&lit| lit != other_lit)
          .find(|lit| assns[lit.var()].is_none());
        match next {
          None => units.push((other_lit, cref)),
          Some(&next) => todo.push((other_lit, cref, next)),
        }
      });
    }
    for (o_lit, cref) in units {
      remove(&mut self.occurrences, o_lit, &cref);
      self.units.insert(o_lit, cref);
    }
    for (prev_watched, cref, next) in todo {
      assert_ne!(self.occurrences[prev_watched.raw() as usize].remove(&cref), None);
      self.occurrences[next.raw() as usize].insert(cref, prev_watched);
    }
  }
  // marks a literal as true, removing it from the set of watched clauses
  fn set_true(&mut self, lit: Literal) {
    self
      .occurrences
      .get_mut(lit.raw() as usize)
      .map(|clauses| clauses.drain().collect::<Vec<_>>())
      .map(|to_rm| {
        to_rm
          .into_iter()
          .for_each(|(cref, lit)| remove(&mut self.occurrences, lit, &cref));
      });
  }
}
