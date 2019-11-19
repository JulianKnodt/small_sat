use crate::{
  database::{ClauseDatabase, ClauseRef},
  literal::Literal,
};

use std::collections::HashMap;

/// An implementation of occurrence lists based on MiniSat's OccList
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
  /// and implication.
  pub(crate) fn add_learnt(
    &mut self,
    assns: &Vec<Option<bool>>,
    cref: &ClauseRef,
    db: &ClauseDatabase,
  ) -> Literal {
    let clause = db.borrow_clause(cref);
    if clause.literals.len() == 1 {
      return clause.literals[0];
    }
    let (false_lit, unassn) =
      db.borrow_clause(cref)
        .literals
        .iter()
        .fold((None, None), |(false_lit, unassn), next| {
          match next.assn(assns) {
            Some(true) => panic!("Unexpected state, found true assignment"),
            Some(false) => (Some(next), unassn),
            None if unassn != None => panic!("Unexpected state multi unassigned assignments"),
            None => (false_lit, Some(next)),
          }
        });
    let unassn = *unassn.expect("Unexpected state, no unassigned lit in learnt clause");
    let false_lit =
      *false_lit.unwrap_or_else(|| panic!("No false lit in clause {:?}", db.borrow_clause(cref)));
    if self.occurrences[unassn.raw() as usize].contains_key(&cref) {
      return unassn;
    }
    assert_eq!(
      self.occurrences[false_lit.raw() as usize].insert(cref.clone(), unassn),
      None
    );
    assert_eq!(
      self.occurrences[unassn.raw() as usize].insert(cref.clone(), false_lit),
      None
    );
    unassn
  }
  pub fn set(
    &mut self,
    lit: Literal,
    assns: &Vec<Option<bool>>,
    clause_db: &ClauseDatabase,
  ) -> Vec<(ClauseRef, Literal)> {
    // Sanity check that we actually assigned this variable
    assert_eq!(lit.assn(assns), Some(true));
    self.set_false(!lit, assns, clause_db)
  }
  /// Sets a given literal to false in this watch list
  fn set_false(
    &mut self,
    lit: Literal,
    assns: &Vec<Option<bool>>,
    clause_db: &ClauseDatabase,
  ) -> Vec<(ClauseRef, Literal)> {
    let clauses = match self.occurrences.get_mut(lit.raw() as usize) {
      // If there were no literals being watched for this, there must be no implications
      None => return vec![],
      Some(clauses) => clauses,
    };

    // TODO remove items from the list without draining
    clauses
      .drain()
      .collect::<Vec<_>>()
      .into_iter()
      .filter_map(|(cref, o_lit)| {
        // If the other one is set to true, we shouldn't update the watch list
        if o_lit.assn(assns) == Some(true) {
          assert_eq!(
            self.occurrences[lit.raw() as usize].insert(cref.clone(), o_lit),
            None
          );
          assert_eq!(self.occurrences[lit.raw() as usize][&cref], o_lit);
          assert_eq!(self.occurrences[o_lit.raw() as usize][&cref], lit);
          assert_ne!(o_lit, lit);
          return None;
        }
        let literals = &clause_db.borrow_clause(&cref).literals;
        let next = literals
          .iter()
          .filter(|&&lit| lit != o_lit)
          .find(|lit| lit.assn(assns) == Some(true))
          .or_else(|| {
            literals
              .iter()
              .filter(|&&lit| lit != o_lit)
              .find(|lit| lit.assn(assns) == None)
          });
        match next {
          // In the case of none, then it implies this is a unit clause,
          // so return it and the literal that needs to be set in it.
          None => {
            // add it back because we need to keep two references in this watch list
            self.occurrences[lit.raw() as usize].insert(cref.clone(), o_lit);
            assert_eq!(self.occurrences[lit.raw() as usize][&cref], o_lit);
            assert_eq!(self.occurrences[o_lit.raw() as usize][&cref], lit);
            assert_ne!(o_lit, lit);
            Some((cref, o_lit))
          },
          Some(&next) => {
            *self.occurrences[o_lit.raw() as usize]
              .get_mut(&cref)
              .unwrap() = next;
            self.occurrences[next.raw() as usize].insert(cref.clone(), o_lit);
            assert_eq!(self.occurrences[next.raw() as usize][&cref], o_lit);
            assert_eq!(self.occurrences[o_lit.raw() as usize][&cref], next);
            assert_ne!(o_lit, next);
            assert!(next.assn(assns) != Some(false));
            None
          },
        }
      })
      .collect()
  }

  /*
  // Checks that watch list invariants hold
  fn assert_ok(&self) {
    self.occurrences.iter().enumerate().for_each(|(lit, watches)| {
      watches.iter().for_each(|(cref, other_lit)| {
        assert_ne!(lit, other_lit.raw() as usize);
        assert_eq!(self.occurrences[other_lit.raw() as usize][cref].raw() as usize, lit);
      })
    })
  }
  */
}
