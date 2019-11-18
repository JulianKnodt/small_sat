use crate::{
  clause::Clause,
  database::{ClauseDatabase, ClauseRef},
  literal::Literal,
  watch_list::WatchList,
};
use std::{collections::HashSet, io, sync::Arc};

#[derive(Clone, Debug)]
pub struct Solver {
  /// which vars are assigned to what at the current stage
  assignments: Vec<Option<bool>>,

  /// stack of assignments, needed for backtracking
  assignment_trail: Vec<Literal>,

  /// keeps track of which level a variable was assigned at
  levels: Vec<Option<usize>>,

  /// keeps track of which clause caused a variable to be assigned.
  /// None in the case of unassigned or assumption
  causes: Vec<Option<ClauseRef>>,

  /// the maximum variable number
  max_var: usize,

  /// Shared Clause Database for this solver
  clause_db: Arc<ClauseDatabase>,

  /// Watch list for this solver, and where list of clauses is kept
  watch_list: WatchList,

  /// which level is this solver currently at
  level: usize,
}

impl Solver {
  /// Attempt to find a satisfying assignment for the current solver
  pub fn solve(&mut self) -> Option<Vec<bool>> {
    assert_eq!(self.level, 0);
    while self.has_unassigned_vars() {
      self.next_level();
      let mut propogation = self.with(self.choose_lit(), None);
      while let Err(clause) = propogation {
        if self.level == 0 {
          return None;
        }
        let (learnt_clause, backtrack_lvl) = self.analyze(&clause, self.level);
        self.backtrack_to(backtrack_lvl);
        // TODO add broadcasting and learning here
        let (cref, lit) = match self.add_learnt_clause(learnt_clause) {
          Err(()) => return None,
          Ok((cref, lit)) => (cref, lit),
        };
        propogation = self.with(lit, Some(cref));
      }
    }
    Some(self.assignments.iter().flat_map(|&i| i).collect())
  }
  pub fn add_learnt_clause(&mut self, c: Clause) -> Result<(ClauseRef, Literal), ()> {
    let len = c.literals.len();
    if len == 0 {
      return Err(());
    }
    let cref = Arc::new(c);
    // add a weaker version of this clause to the shared database
    self.clause_db.add_learnt(Arc::downgrade(&cref));
    let cref = ClauseRef::from(cref);
    if len == 1 {
      let lit = self.clause_db.borrow_clause(&cref).literals[0];
      return Ok((cref, lit));
    }
    let lit = self
      .watch_list
      .add_learnt(&self.assignments, &cref, &self.clause_db);
    return Ok((cref, lit));
  }
  pub fn has_unassigned_vars(&self) -> bool {
    self
      .assignments
      .iter()
      .filter(|assn| assn.is_none())
      .count()
      > 0
  }
  pub fn reason(&self, var: usize) -> Option<&ClauseRef> { self.causes[var].as_ref() }
  /// Analyzes a conflict for a given variable
  pub fn analyze(&self, src_clause: &ClauseRef, decision_level: usize) -> (Clause, usize) {
    let mut learnt: Vec<Literal> = Vec::with_capacity(1);
    let mut seen: HashSet<usize> = HashSet::new();
    let mut learn_until_uip =
      |cref: &ClauseRef, remaining: u32, trail_idx: usize, previous_lit: Option<Literal>| {
        let count: u32 = self
          .clause_db
          .borrow_clause(cref)
          .literals
          .iter()
          // only find new literals
          .filter(|&it| previous_lit != Some(*it))
          .map(|lit| match &self.levels[lit.var()] {
            Some(0) => 0,
            Some(lvl) if !seen.contains(&lit.var()) => {
              seen.insert(lit.var());
              if *lvl >= decision_level {
                1
              } else {
                learnt.push(*lit);
                0
              }
            },
            _ => 0,
          })
          .sum();
        let mut idx = trail_idx;
        while !seen.contains(&self.assignment_trail[idx].var()) && idx > 0 {
          idx = idx - 1;
        }
        let lit_on_path = self.assignment_trail[idx];
        // should have previously seen this assignment
        assert!(seen.remove(&lit_on_path.var()));
        let conflict = self.reason(lit_on_path.var());
        let next_remaining: u32 = (remaining + count).saturating_sub(1);
        (conflict, next_remaining, idx.saturating_sub(1), lit_on_path)
      };
    let mut causes = learn_until_uip(src_clause, 0, self.assignment_trail.len() - 1, None);
    while causes.1 > 0 {
      let conflict = causes
        .0
        .unwrap_or_else(|| panic!("Internal error, got no reason for implication: {:?}", self));
      causes = learn_until_uip(&conflict, causes.1, causes.2, Some(causes.3));
    }
    learnt.push(!causes.3);
    if learnt.len() == 1 {
      // backtrack to 0
      return (Clause::from(learnt), 0);
    }
    // TODO possibly optimize for single pass?
    learnt.sort_by_cached_key(|lit| self.levels[lit.var()].unwrap());
    let max = self.levels[learnt[0].var()].unwrap();
    match learnt
      .iter()
      .filter(|lit| self.levels[lit.var()].unwrap() == max)
      .next()
    {
      None => (Clause::from(learnt), max),
      Some(lit) => {
        let level = self.levels[lit.var()].unwrap();
        (Clause::from(learnt), level)
      },
    }
  }
  /// Records a variable written at the current level
  /// with the given value. This is to only be used when a value is chosen,
  /// Not for implications.
  fn with(&mut self, lit: Literal, cause: Option<ClauseRef>) -> Result<(), ClauseRef> {
    if let Some(cref) = cause {
      if self.assignments[lit.var()] != None {
        return Err(cref);
      }
      assert!(self.causes[lit.var()].replace(cref).is_none());
    }
    self.assignment_trail.push(lit);
    assert_eq!(self.levels[lit.var()].replace(self.level), None);
    assert_eq!(self.assignments[lit.var()].replace(lit.val()), None);
    let assns = self
      .watch_list
      .set(lit, &mut self.assignments, &self.clause_db);
    self.apply_assignments(assns)
  }
  pub fn next_level(&mut self) -> usize {
    self.level += 1;
    self.level
  }
  /// revert to given level, retaining all state at that level.
  /// returns the number of variables removed.
  fn backtrack_to(&mut self, lvl: usize) -> usize {
    self.level = lvl;
    let mut count = 0;
    for var in 0..self.levels.len() {
      if self.levels[var].map_or(false, |assn_lvl| assn_lvl > lvl) {
        assert_ne!(self.assignments[var].take(), None);
        assert_ne!(self.levels[var].take(), None);
        assert_ne!(self.assignment_trail.pop(), None);
        // cannot assert that every variable has a cause, might've randomly selected
        self.causes[var].take();
        count += 1;
      }
    }
    count
  }
  /// simplifies the current set of clauses for this solver
  pub fn simplify() { unimplemented!() }
  pub fn from_dimacs<S: AsRef<std::path::Path>>(s: S) -> io::Result<Self> {
    use crate::dimacs::from_dimacs;
    let (clauses, max_var) = from_dimacs(s)?;
    let clause_db = ClauseDatabase::from(clauses);
    let (wl, units) = WatchList::new(&clause_db);
    assert_eq!(
      max_var, clause_db.max_vars,
      "DIMACS file had incorrect max variable, given {}, computed: {}",
      max_var, clause_db.max_vars
    );
    let mut solver = Self {
      assignments: vec![None; max_var],
      causes: vec![None; max_var],
      assignment_trail: vec![],
      levels: vec![None; max_var],
      max_var: max_var,
      watch_list: wl,
      clause_db: Arc::new(clause_db),
      level: 0,
    };
    solver.apply_assignments(units).expect("UNSAT");
    Ok(solver)
  }
  // TODO decide whether the output should be a single literal or multiple
  fn apply_assignments(
    &mut self,
    unit_implications: Vec<(ClauseRef, Literal)>,
  ) -> Result<(), ClauseRef> {
    let result = unit_implications
      .into_iter()
      .find_map(|(cref, lit)| self.with(lit, Some(cref)).err());
    match result {
      Some(conflict) => Err(conflict),
      None => Ok(()),
    }
  }
  // which literal will be chosen next
  pub fn choose_lit(&self) -> Literal {
    // naive strategy of picking first unassigned one
    let var = self
      .assignments
      .iter()
      .position(|v| v.is_none())
      .expect("No unassigned variables");
    Literal::new(var as u32, true)
  }
  /// resets the solver to it's initial state, retaining learnt clauses
  // TODO reset any heuristics
  pub fn reset_assignments(&mut self) { self.backtrack_to(0); }
}
