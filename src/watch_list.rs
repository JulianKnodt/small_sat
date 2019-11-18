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
  // raw literal ->  Vec(Clause being watched, other literal being watched in clause)
  occurrences: Vec<HashMap<ClauseRef, Literal>>,
}

/// leaves enough space for both true and false variables up to max_vars.
fn space_for_all_lits(size: usize) -> usize { (size << 1) + 2 }

impl WatchList {
  /// returns a new watchlist, as well as any unit clauses
  /// from the initial constraints
  pub fn new(db: &ClauseDatabase) -> (Self, Vec<(ClauseRef, Literal)>) {
    let mut wl = Self {
      occurrences: vec![HashMap::new(); space_for_all_lits(db.max_vars)],
    };
    let units = db
      .iter()
      .filter_map(|cref| wl.watch(&cref, db).map(|lit| (cref, lit)))
      .collect();
    // occurrences.iter_mut().for_each(|occ| occ.sort_by_key(|other_ref| other_ref.0))
    (wl, units)
  }
  /// Adds some clause from the given database to this list.
  /// It must not have previously been added to the list.
  fn watch(&mut self, cref: &ClauseRef, db: &ClauseDatabase) -> Option<Literal> {
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
        assert_eq!(
          self.occurrences[lits[0].raw() as usize].insert(cref.clone(), *lits[1]),
          None
        );
        assert_eq!(
          self.occurrences[lits[1].raw() as usize].insert(cref.clone(), *lits[0]),
          None
        );
      },
      _ => unreachable!(),
    }
    None
  }
  /// adds a learnt clause, which is assumed to have at least two literals as well as cause
  /// an implication
  pub(crate) fn add_learnt(
    &mut self,
    assns: &Vec<Option<bool>>,
    cref: &ClauseRef,
    db: &ClauseDatabase,
  ) -> Literal {
    let (false_lits, true_or_unassn): (Vec<&Literal>, Vec<&Literal>) = db
      .borrow_clause(cref)
      .literals
      .iter()
      .partition(|lit| lit.assn(assns) == Some(false));
    assert_eq!(true_or_unassn.len(), 1, "Learnt clause was not unit");
    assert_eq!(true_or_unassn[0].assn(assns), None);
    assert!(!false_lits.is_empty());
    assert_eq!(
      self.occurrences[true_or_unassn[0].raw() as usize].insert(cref.clone(), *false_lits[0]),
      None
    );
    assert_eq!(
      self.occurrences[false_lits[0].raw() as usize].insert(cref.clone(), *true_or_unassn[0]),
      None
    );
    *true_or_unassn[0]
  }
  pub fn set(
    &mut self,
    lit: Literal,
    assns: &Vec<Option<bool>>,
    clause_db: &ClauseDatabase,
  ) -> Vec<(ClauseRef, Literal)> {
    // Sanity check that we actually assigned this variable
    assert_eq!(assns[lit.var()], Some(lit.val()));
    let out = self.set_false(!lit, assns, clause_db);
    out
  }
  fn set_false(
    &mut self,
    lit: Literal,
    assns: &Vec<Option<bool>>,
    clause_db: &ClauseDatabase,
  ) -> Vec<(ClauseRef, Literal)> {
    // which references do we need to update
    let clauses = match self.occurrences.get_mut(lit.raw() as usize) {
      None => return vec![],
      Some(clauses) => clauses,
    };
    // TODO figure a way to remove items from the list without draining
    clauses
      .drain()
      .collect::<Vec<_>>()
      .into_iter()
      .filter_map(|(cref, o_lit)| {
        // If the other one is set to true, we don't need to update the watch list
        if o_lit.assn(assns) == Some(true) {
          self.occurrences[lit.raw() as usize].insert(cref, o_lit);
          return None
        }
        let next = clause_db
          .borrow_clause(&cref)
          .literals
          .iter()
          .filter(|&&lit| lit != o_lit)
          // TODO convert the next line to search for trues then falses
          .find(|lit| lit.assn(assns) != Some(false));
        match next {
          // In the case of none, then it implies this is a unit clause,
          // so return it and the literal that needs to be set in it.
          None => {
            // add it back because we need to keep two references
            self.occurrences[lit.raw() as usize].insert(cref.clone(), o_lit);
            Some((cref, o_lit))
          },
          Some(&next) => {
            *self.occurrences[o_lit.raw() as usize]
              .get_mut(&cref)
              .unwrap() = next;
            self.occurrences[next.raw() as usize].insert(cref, o_lit);
            None
          },
        }
      })
      .collect()
  }
}
