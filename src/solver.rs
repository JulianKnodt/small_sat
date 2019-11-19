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
  pub db: Arc<ClauseDatabase>,

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
      let lit = self.choose_lit();
      let mut conflict = self.with(lit, None);
      while let Some(clause) = conflict {
        if self.level == 0 {
          return None;
        }
        let (learnt_clause, backtrack_lvl) = self.analyze(&clause, self.level);
        self.backtrack_to(backtrack_lvl);
        // println!("Learning {}", learnt_clause);

        // TODO add broadcasting and learning from others here
        let (cref, lit) = match self.add_learnt_clause(learnt_clause) {
          Some((cref, lit)) => (cref, lit),
          None => return None,
        };
        // assign resulting literal with the learnt clause as the cause
        conflict = self.with(lit, Some(cref));
        assert_eq!(self.assignments[lit.var()], Some(lit.val()));
      }
    }
    Some(self.final_assignments())
  }
  pub fn final_assignments(&self) -> Vec<bool> {
    self.assignments.iter().map(|&i| i.unwrap()).collect()
  }
  /// adds a learnt clause to this solver
  pub fn add_learnt_clause(&mut self, c: Clause) -> Option<(ClauseRef, Literal)> {
    if c.literals.len() == 0 {
      return None;
    }
    let cref = Arc::new(c);
    // add a weaker version of this clause to the shared database
    self.db.add_learnt(Arc::downgrade(&cref));
    let cref = ClauseRef::from(cref);
    let lit = self
      .watch_list
      .add_learnt(&self.assignments, &cref, &self.db);
    return Some((cref, lit));
  }
  // TODO convert this into an integer check instead of linear time
  pub fn has_unassigned_vars(&self) -> bool { self.assignment_trail.len() < self.assignments.len() }
  pub fn reason(&self, var: usize) -> Option<&ClauseRef> { self.causes[var].as_ref() }
  /// Analyzes a conflict for a given variable
  pub fn analyze(&self, src_clause: &ClauseRef, decision_level: usize) -> (Clause, usize) {
    let mut learnt: Vec<Literal> = vec![];
    let mut seen: HashSet<usize> = HashSet::new();
    let mut learn_until_uip =
      |cref: &ClauseRef, remaining: u32, trail_idx: usize, previous_lit: Option<Literal>| {
        let count: u32 = self
          .db
          .borrow_clause(cref)
          .literals
          .iter()
          // only find new literals
          .filter(|&lit| previous_lit != Some(*lit))
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
    let mut levels: Vec<_> = learnt
      .iter()
      .filter_map(|lit| self.levels[lit.var()])
      .collect();
    levels.sort_unstable();
    levels.dedup();
    let max = levels.pop().unwrap();
    match levels.pop() {
      None => (Clause::from(learnt), max),
      Some(level) => (Clause::from(learnt), level),
    }
  }
  pub fn next_level(&mut self) -> usize {
    self.level += 1;
    self.level
  }
  /// revert to given level, retaining all state at that level.
  fn backtrack_to(&mut self, lvl: usize) {
    self.level = lvl;
    for var in 0..self.levels.len() {
      if self.levels[var].map_or(false, |assn_lvl| assn_lvl > lvl) {
        assert_ne!(self.assignments[var].take(), None);
        assert_ne!(self.levels[var].take(), None);
        assert_ne!(self.assignment_trail.pop(), None);
        // cannot assert that every variable has a cause, might've randomly selected
        self.causes[var].take();
      }
    }
    assert_eq!(
      self.assignment_trail.len(),
      self.assignments.iter().filter(|it| it.is_some()).count()
    );
    assert_eq!(
      self.assignment_trail.len(),
      self.levels.iter().filter(|it| it.is_some()).count()
    );
    assert_eq!(
      self.assignment_trail.len(),
      self.assignments.iter().filter(|it| it.is_some()).count(),
    );
  }
  /// simplifies the current set of clauses for this solver
  pub fn simplify() { unimplemented!() }
  pub fn from_dimacs<S: AsRef<std::path::Path>>(s: S) -> io::Result<Self> {
    use crate::dimacs::from_dimacs;
    let (clauses, max_var) = from_dimacs(s)?;
    let db = ClauseDatabase::from(clauses);
    let (wl, units) = WatchList::new(&db);
    assert_eq!(
      max_var, db.max_vars,
      "DIMACS file had incorrect max variable, given {}, computed: {}",
      max_var, db.max_vars
    );
    let mut solver = Self {
      assignments: vec![None; max_var],
      causes: vec![None; max_var],
      assignment_trail: vec![],
      levels: vec![None; max_var],
      max_var: max_var,
      watch_list: wl,
      db: Arc::new(db),
      level: 0,
    };
    for (cause, lit) in units {
      assert_eq!(solver.with(lit, Some(cause)), None, "UNSAT");
    }
    Ok(solver)
  }
  /// Records a literal written at the current level, with a possible cause
  // TODO possibly convert this into two parts which have and don't have causes.
  fn with(&mut self, lit: Literal, cause: Option<ClauseRef>) -> Option<ClauseRef> {
    let mut units = vec![(cause, lit)];
    while let Some((cause, lit)) = units.pop() {
      match lit.assn(&self.assignments) {
        Some(true) => continue,
        None => (),
        Some(false) => return Some(cause.expect("No cause for assigned")),
      }
      self.assignment_trail.push(lit);
      if let Some(cause) = cause {
        assert_eq!(self.causes[lit.var()].replace(cause), None);
      }
      assert_eq!(self.levels[lit.var()].replace(self.level), None);
      assert_eq!(self.assignments[lit.var()].replace(lit.val()), None);
      let new_units = self
        .watch_list
        .set(lit, &mut self.assignments, &self.db)
        .into_iter()
        .map(|(cause, lit)| (Some(cause), lit));
      // does order matter here? dunno fuc it
      units.extend(new_units);
    }
    None
  }
  // which literal will be chosen next
  pub fn choose_lit(&self) -> Literal {
    // naive strategy of picking first unassigned one
    let var = self
      .assignments
      .iter()
      .position(|v| v.is_none())
      .expect("No unassigned variables");
    Literal::new(var as u32, false)
  }
}
