use crate::{
  clause::Clause,
  database::{ClauseDatabase, ClauseRef},
  literal::Literal,
  luby::RestartState,
  stats::{Record, Stats},
  var_state::VariableState,
  watch_list::WatchList,
};
use hashbrown::HashMap;
use std::{cell::RefCell, sync::Arc};

pub const RESTART_BASE: u64 = 100;
pub const RESTART_INC: u64 = 2;
pub const LEARNTSIZE_FACTOR: f64 = 1.0 / 3.0;
pub const LEARNTSIZE_INC: f64 = 1.3;

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

  /// Which index in the assignment trail was a variable assigned at
  level_indeces: Vec<usize>,

  /// Restart State using Luby
  restart_state: RestartState,

  // a reusable stack from lit redundant
  // should be clear before and after each call to lit redundant
  analyze_stack: RefCell<Vec<Literal>>,

  // a reusable tracker for what was seen and what was not
  // should be clear before and after each call to analyze
  analyze_seen: RefCell<HashMap<usize, SeenState>>,

  /// Statistics for this solver
  pub stats: Stats,
}

impl Solver {
  /// Attempt to find a satisfying assignment for the current solver
  pub fn solve(&mut self) -> Option<Vec<bool>> {
    assert_eq!(self.level, 0);
    let mut unsolved_buffer = vec![];
    let mut to_write_buffer = vec![];
    let mut max_learnts = (self.db.initial().len() as f64) * LEARNTSIZE_FACTOR;

    while self.has_unassigned_vars() {
      self.next_level();
      let lit = self.choose_lit();
      let mut conflict = self.with(lit, None);
      while let Some(clause) = conflict {
        self.restart_state.notify_conflict();
        if self.level == 0 {
          self.db.add_solution(None);
          return None;
        }
        if let Some(sol) = self.db.get_solution() {
          return sol;
        }
        self.stats.record(Record::LearnedClause);
        let (learnt_clause, backtrack_lvl) = self.analyze(&clause, self.level);
        assert!(backtrack_lvl < self.level);
        self.backtrack_to(backtrack_lvl);
        if learnt_clause.is_empty() {
          return None;
        }
        self
          .stats
          .record(Record::LearntLiterals(learnt_clause.literals.len()));
        let cref = ClauseRef::from(learnt_clause);
        to_write_buffer.push(cref.clone());
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
          let original_len = unsolved_buffer.len();
          self
            .db
            .since(&mut unsolved_buffer, &mut self.latest_clauses);
          self
            .stats
            .record(Record::Transferred(unsolved_buffer.len() - original_len));
          // TODO need to make it so that can add more than one transfer at the same time?
          while let Some(transfer) = unsolved_buffer.pop() {
            if let Some(sol) = self.db.get_solution() {
              return sol;
            }
            conflict = self.add_transfer(transfer);
            if conflict.is_some() {
              break;
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
      // compacting (currently leads to slow down so probably don't want to compact)
      self.db.compact(self.id);
      if self.stats.clauses_learned + self.stats.transferred_clauses > (max_learnts as usize) {
        self.watch_list.clean(&self.assignments, &self.causes);
        max_learnts *= LEARNTSIZE_INC;
      }
    }
    let solution = self.final_assignments();
    self.db.add_solution(Some(solution.clone()));
    self.stats.rate(std::time::Duration::from_secs(1));
    Some(solution)
  }

  fn add_transfer(&mut self, transfer: ClauseRef) -> Option<ClauseRef> {
    let transfer_conf =
      self
        .watch_list
        .add_transfer(&self.assignments, &self.causes, &self.levels, &transfer);
    if let Some(next_lit) = transfer_conf {
      if let Some(lvl) = self.levels[next_lit.var()] {
        self.backtrack_to(lvl.saturating_sub(1));
        return self.add_transfer(transfer);
      }
      return self.with(next_lit, Some(transfer));
    }
    None
  }

  /// gets the final assignments for this solver
  /// panics if any variable is still null.
  pub fn final_assignments(&self) -> Vec<bool> {
    self.assignments.iter().map(|&i| i.unwrap()).collect()
  }
  /// returns whether there are still unassigned variables for
  /// this solver.
  pub fn has_unassigned_vars(&self) -> bool { self.assignment_trail.len() < self.assignments.len() }
  /// returns the reason for a var's assignment if it exists
  pub fn reason(&self, var: usize) -> Option<&ClauseRef> { self.causes[var].as_ref() }
  /// Analyzes a conflict for a given variable
  fn analyze(&mut self, src_clause: &ClauseRef, decision_level: usize) -> (Clause, usize) {
    let mut learnt: Vec<Literal> = vec![];
    // TODO convert seen, removable, to reused vectors?
    // let mut seen: HashSet<usize> = HashSet::new();
    let mut seen = self.analyze_seen.borrow_mut();
    let curr_len = self.assignment_trail.len() - 1;
    let var_state = &mut self.var_state;
    let levels = &self.levels;
    let trail = &self.assignment_trail;
    let causes = &self.causes;
    let mut learn_until_uip =
      |cref: &ClauseRef, remaining: usize, trail_idx: usize, previous_lit: Option<Literal>| {
        cref.boost();
        let count: usize = cref
          .literals
          .iter()
          // only find new literals
          .filter(|&lit| previous_lit != Some(*lit))
          .filter(|&lit| match &levels[lit.var()] {
            Some(0) => false,
            Some(lvl) if !seen.contains_key(&lit.var()) => {
              seen.insert(lit.var(), SeenState::Source);
              var_state.increase_var_activity(lit.var());
              if *lvl >= decision_level {
                true
              } else {
                learnt.push(*lit);
                false
              }
            },
            _ => false,
          })
          .count();
        let mut idx = trail_idx;
        while !seen.contains_key(&trail[idx].var()) && idx > 0 {
          idx -= 1;
        }
        let lit_on_path = trail[idx];
        // should have previously seen this assignment
        assert!(seen.remove(&lit_on_path.var()).is_some());
        let conflict = causes[lit_on_path.var()].as_ref();
        // self.reason(lit_on_path.var());
        let next_remaining: usize = (remaining + count).saturating_sub(1);
        (conflict, next_remaining, idx.saturating_sub(1), lit_on_path)
      };
    let mut causes = learn_until_uip(src_clause, 0, curr_len, None);
    while causes.1 > 0 {
      let conflict = causes.0.expect("No cause found in analyze?");
      causes = learn_until_uip(&conflict, causes.1, causes.2, Some(causes.3));
    }
    // minimization before adding asserting literal
    // learnt.retain(|lit| self.causes[lit.var()].is_none() || !self.lit_redundant(*lit, &mut seen));

    // add asserting literal
    learnt.push(!causes.3);
    seen.clear();
    if learnt.len() == 1 {
      // backtrack to 0
      return (Clause::from(learnt), 0);
    }
    let mut levels = learnt.iter().filter_map(|lit| self.levels[lit.var()]);
    let curr_max = levels.next().unwrap();
    let (max, second) = levels.fold((curr_max, None), |(max, second), next| {
      use std::cmp::Ordering;
      match next.cmp(&max) {
        Ordering::Greater => (next, Some(max)),
        Ordering::Equal => (max, second),
        Ordering::Less => (max, second.filter(|&v| v >= next).or(Some(next))),
      }
    });
    (Clause::from(learnt), second.unwrap_or(max))
  }
  pub fn next_level(&mut self) -> usize {
    self.level_indeces.push(self.assignment_trail.len());
    self.level += 1;
    self.level
  }
  /// revert to given level, retaining all state at that level.
  fn backtrack_to(&mut self, lvl: usize) {
    if lvl >= self.level {
      return;
    }
    self.level = lvl;
    let index = self.level_indeces[lvl];
    drop(self.level_indeces.drain(lvl..));
    for lit in self.assignment_trail.drain(index..) {
      let var = lit.var();
      assert_ne!(self.assignments[var].take(), None);
      assert_ne!(self.levels[var].take(), None);
      self.polarities[var] = lit.val();
      self.causes[var].take();
      self.var_state.enable(var);
    }
    assert_eq!(self.level_indeces.len(), lvl);
  }
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
      level_indeces: vec![],
      levels: vec![None; max_var],
      watch_list: wl,
      polarities: vec![false; max_var],
      var_state,
      latest_clauses: vec![0; db.num_solvers()],
      db: Arc::new(db),
      level: 0,
      restart_state: RestartState::new(RESTART_BASE, RESTART_INC),
      stats: Stats::new(),
      analyze_stack: RefCell::new(vec![]),
      analyze_seen: RefCell::new(HashMap::new()),
    };
    for (cause, lit) in units {
      assert_eq!(solver.with(lit, Some(cause.clone())), None, "UNSAT");
    }
    Ok(solver)
  }
  /// Records a literal written at the current level, with a possible cause
  fn with(&mut self, lit: Literal, cause: Option<ClauseRef>) -> Option<ClauseRef> {
    let mut units = match cause {
      // In the case there was no previous cause, we need to do one iteration
      None => {
        let mut units = vec![];
        assert!(lit.assn(&self.assignments).is_none());
        self.assignment_trail.push(lit);
        assert_eq!(self.levels[lit.var()].replace(self.level), None);
        assert_eq!(self.assignments[lit.var()].replace(lit.val()), None);
        self.watch_list.set(lit, &self.assignments, &mut units);
        units
      },
      Some(cause) => vec![(cause, lit)],
    };
    while let Some((cause, lit)) = units.pop() {
      match lit.assn(&self.assignments) {
        Some(true) => continue,
        None => (),
        Some(false) => return Some(cause),
      }
      self.assignment_trail.push(lit);
      self.stats.record(Record::Propogation);
      assert_eq!(self.causes[lit.var()].replace(cause), None);
      assert_eq!(self.levels[lit.var()].replace(self.level), None);
      assert_eq!(self.assignments[lit.var()].replace(lit.val()), None);
      self.watch_list.set(lit, &self.assignments, &mut units)
    }
    None
  }
  /// Chooese the next decision literal.
  /// Must take a mutable reference because it must modify the heap of assignments
  fn choose_lit(&mut self) -> Literal {
    assert!(self.has_unassigned_vars());
    let var = loop {
      let next = self.var_state.take_highest_prio();
      if self.assignments[next].is_none() {
        break next;
      }
    };
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
    let db = Arc::get_mut(&mut self.db)?;
    self.latest_clauses = vec![0; n];
    db.resize_to(n);
    let mut replicas = (0..n - 1).map(|_| self.duplicate()).collect::<Vec<_>>();
    replicas.push(self);
    Some(replicas)
  }

  // TODO make this closer to minisat because it's a big source of
  // inefficiency and also might be unsound
  /// checks whether a literal in a conflict clause is redundant
  #[allow(dead_code)]
  fn lit_redundant(&self, lit: Literal, seen: &mut HashMap<usize, SeenState>) -> bool {
    use hashbrown::HashSet;
    assert!(!seen.contains_key(&lit.var()) ^ (seen[&lit.var()] == SeenState::Source));
    let mut remaining = self.analyze_stack.borrow_mut();
    assert!(remaining.is_empty());
    let mut prev = HashSet::new();
    remaining.push(lit);
    while let Some(curr) = remaining.pop() {
      let clause = self.reason(curr.var()).unwrap();
      let lits = clause
        .literals
        .iter()
        // ignore asserting literals
        .filter(|lit| {
          self
            .reason(lit.var())
            .map_or(true, |reason| !Arc::ptr_eq(&reason.inner, &clause.inner))
        });
      for lit in lits {
        let prev_removable = self.levels[lit.var()] == Some(0)
          || prev.contains(&lit.var())
          || seen.get(&lit.var()).map_or(false, |&ss| {
            ss == SeenState::Source || ss == SeenState::Redundant
          });
        if prev_removable {
          continue;
        }
        if self.reason(lit.var()) == None
          || seen
            .get(&lit.var())
            .map_or(false, |&ss| ss == SeenState::Required)
        {
          remaining
            .drain(..)
            .chain(std::iter::once(*lit))
            .chain(std::iter::once(curr))
            .for_each(|lit| {
              seen.entry(lit.var()).or_insert(SeenState::Required);
            });
          return false;
        }
        remaining.push(*lit);
        prev.insert(lit.var());
      }
      seen.entry(lit.var()).or_insert(SeenState::Redundant);
    }
    true
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SeenState {
  Source,
  Redundant,
  Required,
}
