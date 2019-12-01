use crate::{
  database::{ClauseDatabase, ClauseRef},
  literal::Literal,
};

use hashbrown::HashMap;

/// An implementation of occurrence lists based on MiniSat's OccList
#[derive(Clone, Debug, PartialEq)]
pub struct WatchList {
  // raw literal ->  Vec(Clause being watched, other literal being watched in clause)
  occurrences: Vec<HashMap<ClauseRef, Literal>>,
}

/// leaves enough space for both true and false variables up to max_var.
fn space_for_all_lits(size: usize) -> usize { (size << 1) }

impl WatchList {
  /// returns a new watchlist, as well as any unit clauses
  /// from the initial constraints
  pub fn new(db: &ClauseDatabase) -> (Self, Vec<(ClauseRef, Literal)>) {
    let mut wl = Self {
      occurrences: vec![HashMap::new(); space_for_all_lits(db.max_var)],
    };
    let units = db
      .iter()
      .filter_map(|cref| wl.watch(&cref).map(|lit| (cref, lit)))
      .collect();
    (wl, units)
  }
  /// Adds some clause from the given database to this list.
  /// It must not have previously been added to the list.
  fn watch(&mut self, cref: &ClauseRef) -> Option<Literal> {
    let lits = cref.literals.iter().take(2).collect::<Vec<_>>();
    match lits.len() {
      0 => panic!("Empty clause passed to watch"),
      1 => Some(*lits[0]),
      2 => {
        assert!(self.add_clause_with_lits(cref.clone(), *lits[0], *lits[1]));
        None
      },
      _ => unreachable!(),
    }
  }
  /// adds a learnt clause, which is assumed to have at least two literals as well as cause
  /// and implication.
  pub(crate) fn add_learnt(&mut self, assns: &Vec<Option<bool>>, cref: &ClauseRef) -> Literal {
    if cref.literals.len() == 1 {
      return cref.literals[0];
    }
    let mut false_lit = None;
    let mut unassn = None;
    for next in &cref.literals {
      match next.assn(assns) {
        Some(true) => panic!("Unexpected state, found true assignment"),
        Some(false) => false_lit.replace(*next),
        None if unassn.is_some() => panic!("Unexpected state multiple unassigned literals"),
        None => unassn.replace(*next),
      };
    }
    let unassn = unassn.unwrap();
    let false_lit = false_lit.unwrap();
    if !self.occurrences[unassn.raw() as usize].contains_key(&cref) {
      assert!(self.add_clause_with_lits(cref.clone(), false_lit, unassn));
    }
    unassn
  }
  pub fn set(&mut self, lit: Literal, assns: &Vec<Option<bool>>) -> Vec<(ClauseRef, Literal)> {
    // Sanity check that we actually assigned this variable
    assert_eq!(lit.assn(assns), Some(true));
    self.set_false(!lit, assns)
  }
  /// Sets a given literal to false in this watch list
  fn set_false(&mut self, lit: Literal, assns: &Vec<Option<bool>>) -> Vec<(ClauseRef, Literal)> {
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
          //       if self.occurrences[o_lit.raw() as usize].get(&cref) == Some(&lit) {
          assert_eq!(
            self.occurrences[lit.raw() as usize].insert(cref.clone(), o_lit),
            None
          );
          assert_eq!(self.occurrences[lit.raw() as usize][&cref], o_lit);
          assert_eq!(self.occurrences[o_lit.raw() as usize][&cref], lit);
          //    }
          assert_ne!(o_lit, lit);
          return None;
        }
        let literals = &cref.literals;
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
            assert_ne!(o_lit, next);
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
  /// Adds a transferred clause to this watchlist.
  /// If all literals are false
  /// - And none have causes => Pick one at random(Maybe one with lowest priority)
  /// - And some have causes => Pick one with highest level
  /// Else if one literal is true, watch true lit and any false
  /// Else if one literal is unassigned, watch it and any false and return it
  /// Else watch unassigneds.
  pub fn add_transfer(
    &mut self,
    assns: &Vec<Option<bool>>,
    causes: &Vec<Option<ClauseRef>>,
    levels: &Vec<Option<usize>>,
    cref: &ClauseRef,
  ) -> Option<Literal> {
    let literals = &cref.literals;
    assert_ne!(0, literals.len());
    if literals.len() == 1 {
      return match literals[0].assn(assns) {
        Some(false) | None => Some(literals[0]),
        Some(true) => None,
      };
    }
    if self.already_exists(cref) {
      return None;
    }
    let (false_lits, other): (Vec<Literal>, Vec<_>) = literals
      .iter()
      .partition(|lit| lit.assn(assns) == Some(false));
    match other.len() {
      0 => {
        let to_backtrack = *false_lits
          .iter()
          .filter(|lit| causes[lit.var()].is_some())
          .max_by_key(|lit| levels[lit.var()])
          .unwrap_or_else(|| {
            false_lits
              .iter()
              .max_by_key(|lit| levels[lit.var()])
              .unwrap()
          });
        let other_false = false_lits
          .into_iter()
          .filter(|&lit| lit != to_backtrack)
          .next()
          .expect("Other lit must exist");
        assert_ne!(to_backtrack, other_false);
        assert!(self.add_clause_with_lits(cref.clone(), to_backtrack, other_false));
        Some(to_backtrack)
      },
      1 => {
        let single = other[0];
        if !self.occurrences[single.raw() as usize].contains_key(&cref) {
          assert!(self.add_clause_with_lits(cref.clone(), single, false_lits[0]));
        }
        single.assn(assns).map_or(Some(single), |_| None)
      },
      _ => {
        assert!(self.add_clause_with_lits(cref.clone(), other[0], other[1]));
        None
      },
    }
  }
  fn already_exists(&self, cref: &ClauseRef) -> bool {
    let existing = cref
      .literals
      .iter()
      .find(|lit| self.occurrences[lit.raw() as usize].contains_key(cref));
    match existing {
      None => false,
      Some(lit) => {
        let next = self.occurrences[lit.raw() as usize][&cref];
        assert_eq!(
          self.occurrences[next.raw() as usize][&cref],
          *lit,
          "Invariant broken"
        );
        true
      },
    }
  }
  /// returns whether this seems to have violated an invariant or not
  #[must_use]
  fn add_clause_with_lits(&mut self, cref: ClauseRef, lit: Literal, o_lit: Literal) -> bool {
    self.occurrences[lit.raw() as usize]
      .insert(cref.clone(), o_lit)
      .is_none()
      && self.occurrences[o_lit.raw() as usize]
        .insert(cref, lit)
        .is_none()
  }

  pub fn remove_satisfied(&mut self, assns: &Vec<Option<bool>>) {
    self
      .occurrences
      .iter_mut()
      .enumerate()
      .for_each(|(lit, watches)| {
        if Literal::from(lit as u32).assn(assns) == Some(true) {
          watches.clear();
        } else {
          watches.retain(|_, other_lit| other_lit.assn(assns) != Some(true));
        }
      });
  }
  /// removes some old clauses from the databse
  pub fn clean(&mut self, assns: &Vec<Option<bool>>, causes: &Vec<Option<ClauseRef>>) {
    self
      .occurrences
      .iter_mut()
      .enumerate()
      .for_each(|(lit, watches)| {
        let lit = Literal::from(lit as u32);
        // keep clauses which are at least binary
        watches.retain(|clause, &mut o_lit| {
          clause.literals.len() <= 2
            || clause.initial
            || clause.locked(lit, assns, causes)
            || clause.locked(o_lit, assns, causes)
        });
      });
  }
}
