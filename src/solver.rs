use crate::{
  clause::Clause,
  database::{ClauseDatabase, ClauseRef},
  literal::Literal,
};
use std::{collections::HashSet, io, sync::Arc};

#[derive(Clone, Debug)]
pub struct Solver {
  // which vars are assigned to what currently
  assignments: Vec<Option<bool>>,

  /// stack of assignments, needed for backtracking
  assignment_trail: Vec<Literal>,

  /// keeps track of which level a variable was assigned at
  levels: Vec<Option<usize>>,

  /// keeps track of which clause caused a variable to be assigned.
  /// None in the case of unassigned or assumption
  causes: Vec<Option<ClauseRef>>,

  // the maximum variable number
  max_var: usize,

  // Shared Clause Database for this solver
  clause_db: Arc<ClauseDatabase>,
  // last_learnt_clause: usize,
  // TODO do we need to track the last learnt clause?

  // which level is this solver currently at
  level: usize,
}

impl Solver {
  pub fn cdcl_solve(&mut self) -> Option<Vec<bool>> {
    assert_eq!(self.level, 0);
    if self.bool_constraint_propogation().is_err() {
      return None;
    }
    while self.has_unassigned_vars() {
      self.next_level();
      self.with(self.choose_lit());
      while let Err(clause) = self.bool_constraint_propogation() {
        if self.level == 0 {
          return None;
        }
        let (learnt_clause, backtrack_lvl) = self.analyze(&clause, self.level);
        println!(
          "Learned {}, backtracking to {}",
          learnt_clause, backtrack_lvl
        );
        self.backtrack_to(backtrack_lvl);
        self.clause_db.add_learnt(learnt_clause);
      }
    }
    Some(self.assignments.iter().flat_map(|&i| i).collect())
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
    // TODO convert to recursive call
    let mut causes = learn_until_uip(src_clause, 0, self.assignment_trail.len() - 1, None);
    while causes.1 > 0 {
      let conflict = causes
        .0
        .expect("Internal error, got no reason for implication");
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
  /// This returns a result, where ok indicates all constraints propogated with no conflicts,
  /// and the error indicates which clause(by index) caused the error.
  #[must_use]
  fn bool_constraint_propogation(&mut self) -> Result<(), ClauseRef> {
    let mut saw_constraint = true;
    unimplemented!();
    /*
    while saw_constraint {
      saw_constraint = false;
      for i in 0..self.clauses.len() {
        match self.clauses[i].state(&self.assignments) {
          ClauseState::SAT | ClauseState::UNDETERMINED => (),
          ClauseState::UNSAT => return Err(i),
          ClauseState::UNIT(&lit) => {
            self.constraint(lit, i).map_err(|_| i)?;
            saw_constraint = true;
          },
        };
      }
    }
    Ok(())
    */
  }
  pub fn satisfies(&self, clause: &Clause) -> bool {
    clause
      .literals
      .iter()
      .any(|lit| lit.assn(&self.assignments) == Some(true))
  }
  /// Records a variable written at the current level
  /// with the given value. This is to only be used when a value is chosen,
  /// Not for implications.
  fn with(&mut self, lit: Literal) {
    self.assignment_trail.push(lit);
    assert_eq!(self.levels[lit.var()].replace(self.level), None);
    assert_eq!(self.assignments[lit.var()].replace(lit.val()), None);
  }
  /// Constrains a given literal, due to some cause.
  pub fn constraint(&mut self, lit: Literal, cause: ClauseRef) -> Result<(), usize> {
    if self.assignments[lit.var()] != None {
      return Err(lit.var());
    }
    self.with(lit);
    assert!(self.causes[lit.var()].replace(cause).is_none());
    Ok(())
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
        assert!(self.assignments[var].take().is_some());
        assert!(self.levels[var].take().is_some());
        self.causes[var].take(); // cannot assert here cause it might have no cause
        count += 1;
      }
    }
    count
  }
  pub fn simplify() { unimplemented!() }
  pub fn from_dimacs<S: AsRef<std::path::Path>>(s: S) -> io::Result<Self> {
    use crate::dimacs::from_dimacs;
    let (clauses, max_var) = from_dimacs(s)?;
    let clause_db = ClauseDatabase::from(clauses);
    Ok(Self {
      assignments: vec![None; max_var],
      assignment_trail: vec![],
      levels: vec![None; max_var],
      causes: vec![None; max_var],
      max_var: max_var,
      clause_db: Arc::new(clause_db),
      level: 0,
    })
  }
  fn choose_lit(&self) -> Literal {
    // naive strategy of picking first unassigned one
    let var = self
      .assignments
      .iter()
      .position(|v| v.is_none())
      .expect("No unassigned variables");
    Literal::new(var as u32, true)
  }
  pub fn reset_assignments(&mut self) {
    for assn in self.assignments.iter_mut() {
      assn.take();
    }
  }
}

/*
// This is a pure dpll implementation, for correctness.
impl Solver {
  /// uses naive dpll solving in order to solve an SAT formula.
  /// Will return a satisfying assignment if there is one, else it will return none.
  pub fn dpll_solve(&mut self) -> Option<Vec<bool>> {
    if self.bool_constraint_propogation().is_err() {
      return None;
    }
    let all_sat = self.all_sat();
    if all_sat {
      return Some(self.assignments.iter().flat_map(|&i| i).collect());
    } else if !self.has_unassigned_vars() {
      return None;
    }

    let old_level = self.level;
    self.next_level();
    // Should always be ok for dpll solver
    let lit = self.choose_lit();
    self.with(lit);
    self.dpll_solve().or_else(|| {
      self.backtrack_to(old_level);
      self.with(!lit);
      self.dpll_solve()
    })
  }
  pub fn all_sat(&self) -> bool { self.clauses.iter().all(|clause| self.satisfies(clause)) }
}
*/
