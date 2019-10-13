use std::{
  fmt::{self, Display},
  hash::Hash,
};

// Defines a literal
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Literal {
  // the variable represented by this literal
  lit: i32,
}

impl Display for Literal {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}{}", if self.negated() { "!" } else { "" }, self.lit)
  }
}

impl Literal {
  // Panics if this variable is not in the vector
  pub fn assn(&self, assignments: &Vec<Option<bool>>) -> Option<bool> {
    assignments[self.var()].map(|val| self.negated() ^ val)
  }
  /// returns the bool which would return true for this literal
  pub(crate) fn true_eval(&self) -> bool { !self.negated() }
  pub fn var(&self) -> usize { (self.lit.abs() as usize) - 1 }
  pub fn negated(&self) -> bool { self.lit < 0 }
}

// Reads a literal from dimacs format
impl From<i32> for Literal {
  fn from(i: i32) -> Self {
    assert_ne!(i, 0);
    Literal { i }
  }
}
