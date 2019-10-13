use crate::{clause::ClauseState, literal::Literal};

#[derive(Clone, Debug, PartialEq)]
pub struct WatchedClause {
  lits: Vec<Literal>,
  watched: (usize, usize),
}

impl WatchedClause {
  pub fn state<'a>(&'a mut self, assns: &Vec<Option<bool>>) -> ClauseState<'a> {
    if self.lits.len() == 0 {
      return ClauseState::UNSAT;
    }
    self.notify_assignment(assns);
    let (a, b) = self.watched;
    match (self.lits[a].assn(assns), self.lits[b].assn(assns)) {
      (Some(true), _) => ClauseState::SAT,
      (_, Some(true)) => ClauseState::SAT,
      (None, None) if a != b => ClauseState::UNDETERMINED,
      (None, None) if a == b => ClauseState::UNIT(&self.lits[a]),
      // The following two should be unreachable
      (None, _) => ClauseState::UNIT(&self.lits[a]),
      (_, None) => ClauseState::UNIT(&self.lits[b]),
      (Some(false), Some(false)) => ClauseState::UNSAT,
    }
  }
  // TODO make this return the newly modified assignments?
  pub(crate) fn notify_assignment(&mut self, assns: &Vec<Option<bool>>) {
    let (hd, tl) = self.watched;
    if assns[hd] == Some(false) {
      self
        .lits
        .iter()
        .position(|lit| lit.assn(assns).is_none())
        .map(|next_hd| self.watched.0 = next_hd);
    }
    if assns[tl] == Some(false) {
      self
        .lits
        .iter()
        .rposition(|lit| lit.assn(assns).is_none())
        .map(|next_tl| self.watched.1 = next_tl);
    }
  }
}

impl From<Vec<Literal>> for WatchedClause {
  fn from(lits: Vec<Literal>) -> Self {
    WatchedClause {
      watched: (0, lits.len() - 1),
      lits: lits,
    }
  }
}

#[cfg(test)]
mod test {
  use super::WatchedClause;
  use crate::{clause::ClauseState, literal::Literal};
  #[test]
  fn test_basic_functionality() {
    let mut watched = WatchedClause::from(vec![
      Literal::from(1),
      Literal::from(2),
      Literal::from(3),
      Literal::from(-4),
    ]);
    let mut assns = vec![None; 4];
    let original = watched.clone();
    assert_eq!(watched.state(&assns), ClauseState::UNDETERMINED);
    assert_eq!(watched, original);
    assns[0] = Some(true);
    assert_eq!(watched.state(&assns), ClauseState::SAT);
    assns[0] = Some(false);
    assns[3] = Some(false);
    assert_eq!(watched.state(&assns), ClauseState::UNDETERMINED);
    assns[1] = Some(false);
    assert_eq!(watched.state(&assns), ClauseState::UNIT(&Literal::from(3)));
  }
}

// TODO build occurrence list to see who is watching what
// This will map to a hashmap of a vector of watched clauses.

// pub struct OccurrenceList<'a> {
//   occs: Vec<Vec<&'a Clause>>,
// }







