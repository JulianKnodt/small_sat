use crate::{
  database::{ClauseDatabase, ClauseRef},
  literal::Literal,
};
use hashbrown::HashMap;
use std::sync::{
  atomic::{AtomicU64, Ordering},
  Arc, Weak,
};

/// An implementation of occurrence lists based on MiniSat's OccList
#[derive(Clone, Debug)]
pub struct WatchList {
  // raw literal ->  Vec(Clause being watched, other literal being watched in clause)
  occurrences: Vec<HashMap<ClauseRef, Literal>>,
  // activities for the clauses in this watchlist
  activities: Vec<Weak<AtomicU64>>,
}

/// leaves enough space for both true and false variables up to max_var.
#[inline]
fn space_for_all_lits(size: usize) -> usize { (size << 1) }

impl WatchList {
  /// returns a new watchlist, as well as any unit clauses
  /// from the initial constraints
  pub fn new(db: &ClauseDatabase) -> (Self, Vec<(ClauseRef, Literal)>) {
    let mut wl = Self {
      occurrences: vec![HashMap::new(); space_for_all_lits(db.max_var)],
      activities: vec![],
    };
    let units = db
      .iter()
      .filter_map(|cref| wl.watch(&cref).map(|lit| (cref, lit)))
      .collect();
    (wl, units)
  }
  /*
  /// Returns the number of items in this watchlist
  pub fn size(&self) -> usize {
    self.occurrences.iter()
      .map(|watches| watches.len())
      .sum::<usize>()/2
  }
  */
  /// Adds some clause from the given database to this list.
  /// It must not have previously been added to the list.
  fn watch(&mut self, cref: &ClauseRef) -> Option<Literal> {
    let mut lits = cref.literals.iter().take(2);
    match lits.next() {
      None => panic!("Empty clause passed to watch"),
      Some(&lit) => match lits.next() {
        None => Some(lit),
        Some(&o_lit) => {
          assert!(self.add_clause_with_lits(cref.clone(), lit, o_lit));
          None
        },
      },
    }
  }
  /// adds a learnt clause, which is assumed to have at least two literals as well as cause
  /// and implication.
  pub(crate) fn add_learnt(&mut self, assns: &[Option<bool>], cref: &ClauseRef) -> Literal {
    if cref.literals.len() == 1 {
      return cref.literals[0];
    }
    self.activities.push(Arc::downgrade(&cref.activity));
    debug_assert!(!cref
      .literals
      .iter()
      .any(|lit| lit.assn(assns) == Some(true)));
    debug_assert_eq!(
      1,
      cref
        .literals
        .iter()
        .filter(|lit| lit.assn(assns).is_none())
        .count()
    );
    let false_lit = *cref
      .literals
      .iter()
      .find(|lit| lit.assn(&assns) == Some(false))
      .unwrap();
    let unassn = *cref
      .literals
      .iter()
      .find(|lit| lit.assn(&assns).is_none())
      .unwrap();
    if !self.occurrences[unassn.raw() as usize].contains_key(&cref) {
      assert!(self.add_clause_with_lits(cref.clone(), false_lit, unassn));
    }
    unassn
  }
  pub fn set<T>(&mut self, lit: Literal, assns: &[Option<bool>], into: &mut T)
  where
    T: Extend<(ClauseRef, Literal)>, {
    // Sanity check that we actually assigned this variable
    assert_eq!(lit.assn(assns), Some(true));
    self.set_false(!lit, assns, into)
  }
  /// Sets a given literal to false in this watch list
  fn set_false<T>(&mut self, lit: Literal, assns: &[Option<bool>], into: &mut T)
  where
    T: Extend<(ClauseRef, Literal)>, {
    use std::mem::swap;
    assert!((lit.raw() as usize) < self.occurrences.len());
    let mut swap_map = HashMap::new();
    swap(&mut self.occurrences[lit.raw() as usize], &mut swap_map);
    // removing items from the list without draining
    // should help improve efficiency
    swap_map.retain(|cref, &mut o_lit| {
      assert_ne!(lit, o_lit);
      // If the other one is set to true, we shouldn't update the watch list
      if o_lit.assn(assns) == Some(true) {
        debug_assert_eq!(self.occurrences[o_lit.raw() as usize][&cref], lit);
        return true;
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
          debug_assert_eq!(self.occurrences[o_lit.raw() as usize][&cref], lit);
          into.extend(std::iter::once((cref.clone(), o_lit)));
          true
        },
        Some(&next) => {
          assert_ne!(lit, next);
          assert_ne!(o_lit, next);
          *self.occurrences[o_lit.raw() as usize]
            .get_mut(&cref)
            .unwrap() = next;
          self.occurrences[next.raw() as usize].insert(cref.clone(), o_lit);
          debug_assert_eq!(self.occurrences[next.raw() as usize][&cref], o_lit);
          debug_assert_eq!(self.occurrences[o_lit.raw() as usize][&cref], next);
          assert!(next.assn(assns) != Some(false));
          false
        },
      }
    });
    swap(&mut self.occurrences[lit.raw() as usize], &mut swap_map);
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
    assns: &[Option<bool>],
    causes: &[Option<ClauseRef>],
    levels: &[Option<usize>],
    cref: &ClauseRef,
  ) -> Option<Literal> {
    let literals = &cref.literals;
    assert!(!literals.is_empty());
    if literals.len() == 1 {
      return match literals[0].assn(assns) {
        Some(false) | None => Some(literals[0]),
        Some(true) => None,
      };
    }
    if self.already_exists(cref) {
      return None;
    }
    self.activities.push(Arc::downgrade(&cref.activity));
    // Only need to track at most 4 literals
    let mut watchable = literals
      .iter()
      .filter(|lit| lit.assn(&assns) != Some(false));
    match watchable.next() {
      None => {
        // this case can cause unsoundness on some rare occasions
        let to_backtrack = *literals
          .iter()
          .filter(|lit| causes[lit.var()].is_some())
          .min_by_key(|lit| levels[lit.var()])
          .unwrap_or_else(|| literals.iter().min_by_key(|lit| levels[lit.var()]).unwrap());
        let other_false = *literals
          .iter()
          .filter(|lit| levels[lit.var()].unwrap() < levels[to_backtrack.var()].unwrap())
          .find(|&&lit| lit != to_backtrack)?;
        debug_assert_ne!(to_backtrack, other_false);
        debug_assert!(levels[to_backtrack.var()] > levels[other_false.var()]);
        assert!(self.add_clause_with_lits(cref.clone(), to_backtrack, other_false));
        Some(to_backtrack)
      },
      Some(&lit) => match watchable.next() {
        None => match lit.assn(assns) {
          // Don't track clauses which have a true literal
          Some(true) => None,
          Some(false) => unreachable!(),
          None => {
            if !self.occurrences[lit.raw() as usize].contains_key(&cref) {
              let other = *literals
                .iter()
                .find(|lit| lit.assn(&assns) == Some(false))?;
              assert!(self.add_clause_with_lits(cref.clone(), lit, other));
            }
            Some(lit)
          },
        },
        Some(&o_lit) => {
          assert!(self.add_clause_with_lits(cref.clone(), lit, o_lit));
          None
        },
      },
    }
  }
  fn already_exists(&self, cref: &ClauseRef) -> bool {
    cref
      .literals
      .iter()
      .any(|lit| self.occurrences[lit.raw() as usize].contains_key(cref))
  }
  /// Adds a clause with the given literals into the watch list.
  /// Returns true if another clause was evicted, which likely implies an invariant
  /// was broken.
  #[must_use]
  fn add_clause_with_lits(&mut self, cref: ClauseRef, lit: Literal, o_lit: Literal) -> bool {
    self.occurrences[lit.raw() as usize]
      .insert(cref.clone(), o_lit)
      .is_none()
      && self.occurrences[o_lit.raw() as usize]
        .insert(cref, lit)
        .is_none()
  }

  pub fn remove_satisfied(&mut self, assns: &[Option<bool>]) {
    // TODO could I swap the ordering here of which lit is being removed
    self
      .occurrences
      .iter_mut()
      .enumerate()
      .filter(|(_, watches)| !watches.is_empty())
      .for_each(|(lit, watches)| {
        if Literal::from(lit as u32).assn(assns) == Some(true) {
          watches.retain(|cref, _| cref.initial);
        } else {
          watches.retain(|cref, other_lit| cref.initial || other_lit.assn(assns) != Some(true));
        }
      });
  }
  /// returns the median activity for this watchlist
  fn median_activity(&mut self) -> Option<u64> {
    let median_position = self.activities.len() / 2;
    self
      .activities
      .partition_at_index_by_key(median_position, |act| {
        act.upgrade().map_or(0, |act| act.load(Ordering::SeqCst))
      })
      .1
      .upgrade()
      .map(|act| act.load(Ordering::SeqCst))
  }
  /// removes some old clauses from the databse
  pub fn clean(&mut self, assns: &[Option<bool>], causes: &[Option<ClauseRef>]) {
    if self.activities.is_empty() {
      return;
    }
    let threshold = match self.median_activity() {
      None => return,
      Some(med) => med,
    };
    let curr: HashMap<ClauseRef, u64> = self
      .occurrences
      .iter_mut()
      .flat_map(|watch| {
        watch
          .keys()
          .map(|cref| (cref.clone(), cref.curr_activity()))
      })
      .collect();
    self
      .occurrences
      .iter_mut()
      .enumerate()
      .filter(|(_, watches)| !watches.is_empty())
      .for_each(|(lit, watches)| {
        let lit = Literal::from(lit as u32);
        // Threshold is the median of all clause activities for this watch list
        watches.retain(|cref, &mut o_lit| {
          cref.literals.len() <= 2
            || cref.initial
            || curr[cref] >= threshold
            || cref.locked(lit, assns, causes)
            || cref.locked(o_lit, assns, causes)
        });
      });
    drop(curr);
    self.activities.retain(|act| act.strong_count() > 0);
  }
}
