use crate::{
  clause::{Clause, ClauseState},
  literal::Literal,
  // conflict_clause::Implication,
};
use std::{fmt, io};

// TODO create a way to check which clauses are causing implications for tracking to find UIP

#[derive(Clone, Debug, PartialEq)]
pub struct Solver {
  // which vars are assigned to what currently
  assignments: Vec<Option<bool>>,

  // stack of implications, needed for backtracking
  // implications: Vec<Implication>,

  level_assigned: Vec<(usize, usize)>,

  // the maximum variable number
  max_var: usize,
  // clauses for this Solver
  clauses: Vec<Clause>,
  // which level is this solver currently at
  level: usize,
}

#[derive(Debug, PartialEq)]
pub struct Conflict {
  causes: Vec<usize>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SATState {
  SAT,
  UNSAT,
  UNDETERMINED,
}

impl Solver {
  /// uses naive dpll solving in order to solve an SAT formula.
  /// Will return a satisfying assignment if there is one, else it will return none.
  pub fn dpll_solve(&mut self) -> Option<Vec<Option<bool>>> {
    self.bool_constraint_propogation();
    self.level += 1;
    let new_level = self.level;
    match self.state() {
      SATState::SAT => Some(self.assignments.clone()),
      SATState::UNSAT => None,
      SATState::UNDETERMINED => {
        let var = self.choose_var();
        self.with(var, true);
        self.dpll_solve().or_else(|| {
          self.backtrack_to(new_level);
          self.with(var, false);
          self.dpll_solve()
        })
      },
    }
  }
  pub fn cdcl_solve(&mut self) -> Option<Vec<Option<bool>>> { unimplemented!() }
  pub fn has_unassigned_vars(&self) -> bool {
    self
      .assignments
      .iter()
      .filter(|assn| assn.is_none())
      .count()
      > 0
  }
  fn bool_constraint_propogation(&mut self) {
    let mut saw_constraint = true;
    while saw_constraint {
      saw_constraint = false;
      (0..self.clauses.len()).for_each(|c| match self.clauses[c].state(&self.assignments) {
        ClauseState::SAT | ClauseState::UNSAT | ClauseState::UNDETERMINED => (),
        ClauseState::UNIT(lit) => {
          let (var, true_eval) = (lit.var, lit.true_eval());
          self.with(var, true_eval);
          saw_constraint = true;
        },
      });
    }
  }
  fn choose_var(&self) -> usize {
    // naive strategy of picking first unassigned one
    self
      .assignments
      .iter()
      .position(|v| v.is_none())
      .expect("No unassigned variables")
  }
  fn with(&mut self, var: usize, val: bool) {
    self.level_assigned.push((self.level, var));
    assert_eq!(None, self.assignments[var]);
    self.assignments[var].replace(val);
  }
  fn backtrack_to(&mut self, lvl: usize) {
    // TODO could use binary search in order to find earliest point but doesn't matter because
    // still could iterate backwards
    while self
      .level_assigned
      .last()
      .map_or(false, |last| last.0 >= lvl)
    {
      let (_, var) = self.level_assigned.pop().unwrap();
      assert!(self.assignments[var].is_some());
      self.assignments[var].take();
    }
  }
  pub fn state(&self) -> SATState {
    let clause_states = self.clauses.iter().map(|c| c.state(&self.assignments));
    for clause_state in clause_states {
      match clause_state {
        ClauseState::SAT => (),
        ClauseState::UNSAT => return SATState::UNSAT,
        ClauseState::UNDETERMINED | ClauseState::UNIT(_) => return SATState::UNDETERMINED,
      }
    }
    SATState::SAT
  }
  pub fn add_empty_clause(&mut self) { unimplemented!() }
  pub fn add_unary_clause(&mut self, l: Literal) { unimplemented!() }
  pub fn add_clause(&mut self, clause: Clause) { self.clauses.push(clause); }
  pub fn simplify() { unimplemented!() }

  /// are you ok?
  pub fn ok() -> bool { unimplemented!() }

  pub fn from_dimacs<S>(s: S) -> io::Result<Self>
  where
    S: AsRef<std::path::Path>, {
    use std::{
      fs::File,
      io::{BufRead, BufReader},
      mem,
    };
    let file = File::open(s)?;
    let buf_reader = BufReader::new(file);
    let mut clauses = vec![];
    let mut max_var = 0;
    let mut curr_lits = vec![];
    for line in buf_reader.lines() {
      let line = line?;
      let line = line.trim();
      if line.starts_with("c") {
        continue;
      } // comments
      if line.starts_with("p cnf") {
        let items = line
          .split_whitespace()
          .filter_map(|v| v.parse::<usize>().ok())
          .collect::<Vec<_>>();
        assert_eq!(items.len(), 2);
        max_var = items[0];
        clauses.reserve(items[1]);
      } else {
        line
          .split_whitespace()
          .map(|v| {
            v.parse::<i32>()
              .expect("Failed to parse int in dimacs file")
          })
          .for_each(|v| match v {
            0 => {
              let complete_clause = mem::replace(&mut curr_lits, vec![]);
              clauses.push(Clause::from(complete_clause));
            },
            v => curr_lits.push(Literal::from(v)),
          });
      }
    }
    Ok(Solver {
      assignments: vec![None; max_var],
      level_assigned: vec![],
      clauses: clauses,
      level: 0,
      max_var: max_var,
    })
  }
}

impl fmt::Display for Solver {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if self.clauses.len() > 0 {
      write!(f, "{}", self.clauses[0])?;
      for clause in self.clauses.iter().skip(1) {
        write!(f, " & {}", clause)?;
      }
    }
    Ok(())
  }
}
