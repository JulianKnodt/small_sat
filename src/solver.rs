use crate::{
  clause::Clause,
  database::{ClauseDatabase, ClauseRef},
  literal::Literal,
  luby::RestartState,
  stats::{Record, Stats},
  var_state::VariableState,
  watch_list::WatchList,
};
use hashbrown::HashSet;
use std::sync::Arc;

pub const RESTART_BASE: u64 = 100;
pub const RESTART_INC: u64 = 2;

#[derive(Clone, Debug)]
pub struct Solver {
  /// The identifiying number for this solver
  id: usize,

  /// which vars are assigned to what at the current stage
  assignments: Vec<Option<bool>>,

  /// stack of assignments, needed for backtracking
  assignment_trail: Vec<Literal>,

  /// keeps track of which level a variable was assigned at
  levels: Vec<Option<usize>>,

  /// keeps track of which clause caused a variable to be assigned.
  /// None in the case of unassigned or assumption
  causes: Vec<Option<ClauseRef>>,

  /// Shared Clause Database for this solver
  pub db: Arc<ClauseDatabase>,

  /// Watch list for this solver, and where list of clauses is kept
  watch_list: WatchList,

  /// last assigned per each variable
  /// initialized to false
  polarities: Vec<bool>,

  /// Var state independent decaying sum
  var_state: VariableState,

  /// vector clock of clauses for database
  latest_clauses: Vec<usize>,

  /// which level is this solver currently at
  level: usize,
  // /// Statistics for this solver
  /// Restart State using Luby
  restart_state: RestartState,

  pub stats: Stats,
}

impl Solver {
  /// Attempt to find a satisfying assignment for the current solver
  pub fn solve(&mut self) -> Option<Vec<bool>> {
    assert_eq!(self.level, 0);
    let mut unsolved_buffer = vec![];
    let mut to_write_buffer = vec![];

    while self.has_unassigned_vars() {
      self.next_level();
      let lit = self.choose_lit();
      let mut conflict = self.with(lit, None);
      while let Some(clause) = conflict {
        self.restart_state.notify_conflict();
        if self.level == 0 {
          return None;
        }
        self.stats.record(Record::LearnedClause);
        let (learnt_clause, backtrack_lvl) = self.analyze(&clause, self.level);
        self.backtrack_to(backtrack_lvl);
        if learnt_clause.literals.len() == 0 {
          return None;
        }
        let cref = Arc::new(learnt_clause);
        to_write_buffer.push(Arc::downgrade(&cref));
        let cref = ClauseRef::from(cref);
        let lit = self.watch_list.add_learnt(&self.assignments, &cref);

        self.var_state.decay();
        // assign resulting literal with the learnt clause as the cause
        conflict = self.with(lit, Some(cref));
        assert_eq!(self.assignments[lit.var()], Some(lit.val()));

        // handle transfers when there are no more conflicts in own clauses
        // but might need to handle conflicts here
        if conflict == None {
          self
            .stats
            .record(Record::Written(to_write_buffer.len() as u32));
          self.latest_clauses[self.id] = self.db.add_learnts(self.id, &mut to_write_buffer);
          assert!(to_write_buffer.is_empty());
          let (new_clauses, new_timestamp) = self.db.since(&self.latest_clauses);
          self
            .stats
            .record(Record::Transferred(new_clauses.len() as u32));
          self.latest_clauses = new_timestamp;
          unsolved_buffer.extend(new_clauses);
          while let Some(transfer) = unsolved_buffer.pop() {
            let transfer_outcome = self.watch_list.add_transfer(
              &self.assignments,
              &self.causes,
              &self.levels,
              &transfer,
            );
            if let Some(next_lit) = transfer_outcome {
              conflict = self.with(next_lit, Some(transfer));
              if conflict.is_some() {
                self.backtrack_to(self.levels[next_lit.var()].unwrap());
                break;
              }
            }
          }
        }
      }
      if self.restart_state.restart_suggested() {
        self.stats.record(Record::Restart);
        self.restart_state.restart();
        self.backtrack_to(0);
      }
      if self.level == 0 {
        self.watch_list.remove_satisfied(&self.assignments);
      }
      // self.watch_list.clean(&self.assignments, &self.causes);
    }
    Some(self.final_assignments())
  }

  /// gets the final assignments for this solver
  /// panicks if one is still null
  pub fn final_assignments(&self) -> Vec<bool> {
    self.assignments.iter().map(|&i| i.unwrap()).collect()
  }
  /// returns whether there are still unassigned variables for
  /// this solver.
  pub fn has_unassigned_vars(&self) -> bool { self.assignment_trail.len() < self.assignments.len() }
  pub fn assigned_vars(&self) -> usize { self.assignment_trail.len() }
  pub fn reason(&self, var: usize) -> Option<&ClauseRef> { self.causes[var].as_ref() }
  /// Analyzes a conflict for a given variable
  pub fn analyze(&mut self, src_clause: &ClauseRef, decision_level: usize) -> (Clause, usize) {
    let mut learnt: Vec<Literal> = vec![];
    let mut seen: HashSet<usize> = HashSet::new();
    let curr_len = self.assignment_trail.len() - 1;
    let mut learn_until_uip =
      |cref: &ClauseRef, remaining: u32, trail_idx: usize, previous_lit: Option<Literal>| {
        let count: u32 = cref
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
    let mut causes = learn_until_uip(src_clause, 0, curr_len, None);
    while causes.1 > 0 {
      let conflict = causes
        .0
        .unwrap_or_else(|| panic!("Internal error, got no reason for implication: {:?}", self));
      causes = learn_until_uip(&conflict, causes.1, causes.2, Some(causes.3));
    }
    // minimization before adding asserting literal
    let mut removable = HashSet::new();
    let mut failed = HashSet::new();
    learnt.retain(|lit| {
      self.causes[lit.var()].is_none()
        || !self.lit_redundant(*lit, &mut seen, &mut removable, &mut failed)
    });
    // add asserting literal
    learnt.push(!causes.3);
    seen
      .drain()
      .chain(removable.drain())
      .chain(failed.drain())
      .for_each(|var| self.var_state.increase_var_activity(var));
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
        let trail = self.assignment_trail.pop().unwrap();
        self.polarities[trail.var()] = trail.val();
        // cannot assert that every variable has a cause, might've decided it
        self.causes[var].take();
        self.var_state.enable(var);
      }
    }
  }
  /// simplifies the current set of clauses for this solver
  pub fn simplify() { unimplemented!() }
  pub fn from_dimacs<S: AsRef<std::path::Path>>(s: S) -> std::io::Result<Self> {
    use crate::dimacs::from_dimacs;
    let (clauses, max_var) = from_dimacs(s)?;
    let db = ClauseDatabase::new(max_var, clauses);
    let (wl, units) = WatchList::new(&db);
    let var_state = VariableState::from(&db);
    let mut solver = Self {
      id: db.next_id(),
      assignments: vec![None; max_var],
      causes: vec![None; max_var],
      assignment_trail: vec![],
      levels: vec![None; max_var],
      watch_list: wl,
      polarities: vec![false; max_var],
      var_state,
      latest_clauses: vec![0; db.learnt_clauses.len()],
      db: Arc::new(db),
      level: 0,
      restart_state: RestartState::new(RESTART_BASE, RESTART_INC),
      stats: Stats::new(),
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
        self.stats.record(Record::Propogation);
        assert_eq!(self.causes[lit.var()].replace(cause), None);
      }
      assert_eq!(self.levels[lit.var()].replace(self.level), None);
      assert_eq!(self.assignments[lit.var()].replace(lit.val()), None);
      let new_units = self
        .watch_list
        .set(lit, &mut self.assignments)
        .into_iter()
        .map(|(cause, lit)| (Some(cause), lit));
      // does order matter here? dunno fuc it
      units.extend(new_units);
    }
    None
  }
  /// Chooese the next decision literal.
  /// Must take a mutable reference because it must modify the heap of assignments
  fn choose_lit(&mut self) -> Literal {
    let var = loop {
      let next = self.var_state.take_highest_prio();
      if self.assignments[next].is_none() {
        break next;
      }
    };
    assert_eq!(self.assignments[var], None);
    // naive strategy of picking first unassigned one
    Literal::new(var as u32, !self.polarities[var])
  }

  /// Clones this solver and increments its id.
  /// If the database cannot have more solvers
  /// returns none.
  pub fn duplicate(&self) -> Self {
    let mut out = self.clone();
    out.id = self.db.next_id();
    out
  }
  pub fn id(&self) -> usize { self.id }
  /// Replicates this one solver into multiple with the same state.
  /// Returns none if replicate was called before.
  pub fn replicate(mut self, n: usize) -> Option<Vec<Self>> {
    self.latest_clauses = vec![0; n];
    let db = Arc::get_mut(&mut self.db)?;
    db.resize_to(n);
    let mut replicas = (0..n - 1).map(|_| self.duplicate()).collect::<Vec<_>>();
    replicas.push(self);
    Some(replicas)
  }
  /// checks whether a literal in a conflict clause is redundant
  fn lit_redundant(
    &self,
    lit: Literal,
    seen: &HashSet<usize>,
    removable: &mut HashSet<usize>,
    failed: &mut HashSet<usize>,
  ) -> bool {
    assert!(!removable.contains(&lit.var()));
    assert!(!failed.contains(&lit.var()));
    let mut remaining = vec![lit];
    while let Some(curr) = remaining.pop() {
      let clause = self
        .reason(curr.var())
        .expect("Failed to find reason in lit_redundant");
      let lits = clause
        .literals
        .iter()
        // ignore asserting literals
        .filter(|lit| {
          self
            .reason(lit.var())
            .map_or(false, |reason| Arc::ptr_eq(&reason.inner, &clause.inner))
        });
      for lit in lits {
        let prev_removable = self.levels[lit.var()] == Some(0)
          || seen.contains(&lit.var())
          || removable.contains(&lit.var());
        if prev_removable {
          continue;
        }
        if self.reason(lit.var()) == None || failed.contains(&lit.var()) {
          remaining
            .into_iter()
            .chain(std::iter::once(*lit))
            .for_each(|lit| {
              if !seen.contains(&lit.var()) {
                failed.insert(lit.var());
              }
            });
          return false;
        }
        remaining.push(*lit);
      }
      if !seen.contains(&lit.var()) {
        removable.insert(lit.var());
      }
    }
    true
  }
}
