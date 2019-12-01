extern crate priority_queue;

use crate::{clause::Clause, database::ClauseDatabase};
use hashbrown::HashMap;
use priority_queue::PriorityQueue;
use ahash::ABuildHasher;

#[derive(PartialOrd, Debug, PartialEq, Clone, Copy)]
struct Priority(f32);

impl Eq for Priority {}
impl Ord for Priority {
  fn cmp(&self, o: &Self) -> std::cmp::Ordering { self.partial_cmp(o).unwrap() }
}

#[derive(Debug, PartialEq, Clone)]
pub struct VariableState {
  // Variable -> activity
  priorities: PriorityQueue<usize, Priority, ABuildHasher>,
  /// buffer for assigned variables
  evicted: HashMap<usize, Priority>,
  /// constant rate of decay for this state
  pub decay_rate: f32,

  /// How much to increment the activity each time a variable is seen
  pub inc_amt: f32,
}

pub const DEFAULT_DECAY_RATE: f32 = 1.2;
pub const DEFAULT_INC_AMT: f32 = 1.0;

impl VariableState {
  /// decays the current occurrence account
  pub fn decay(&mut self) {
    let decay_rate = self.decay_rate;
    self
      .priorities
      .iter_mut()
      .for_each(|(_, v)| v.0 /= decay_rate);
    self.evicted.values_mut().for_each(|v| v.0 /= decay_rate);
  }
  /// Increases the activity for this variable
  pub fn increase_var_activity(&mut self, var: usize) {
    let inc_amt = self.inc_amt;
    self
      .priorities
      .change_priority_by(&var, |p| Priority(p.0 + inc_amt.copysign(p.0)));
  }
  /// Adds a clause to this variable state cache
  pub fn add_clause(&mut self, c: &Clause) {
    c.literals
      .iter()
      .for_each(|lit| self.increase_var_activity(lit.var()));
  }
  pub fn enable(&mut self, var: usize) {
    let prev = self.evicted.remove(&var);
    if let Some(prev) = prev {
      self.priorities.push(var, prev);
    }
  }
  /// returns the variable with highest priority
  /// Modifies the internal state so that the variable cannot be picked again
  /// Until it is re-enabled
  pub fn take_highest_prio(&mut self) -> usize {
    let next = self.priorities.pop().unwrap();
    self.evicted.insert(next.0, next.1);
    next.0
  }
}

impl From<&'_ ClauseDatabase> for VariableState {
  fn from(db: &ClauseDatabase) -> Self {
    let mut priorities = PriorityQueue::with_capacity_and_default_hasher(db.max_var);
    priorities.extend((0..db.max_var)
      .map(|var| (var, Priority(0.0))));
    let mut state = Self {
      priorities,
      evicted: HashMap::new(),
      decay_rate: DEFAULT_DECAY_RATE,
      inc_amt: DEFAULT_INC_AMT,
    };
    db.iter().for_each(|cref| state.add_clause(&cref));
    state
  }
}
