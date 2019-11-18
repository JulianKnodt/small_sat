use std::{
  fmt::{self, Debug, Display},
  hash::Hash,
  ops::Not,
};

// Defines a literal
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Literal(u32);

impl Literal {
  pub fn new(var: u32, negated: bool) -> Self { Self((var << 1) + (negated as u32)) }
  // Panics if this variable is not in the vector
  /// returns the value for this literal given these assignments
  pub fn assn(&self, assignments: &Vec<Option<bool>>) -> Option<bool> {
    assignments[self.var()].map(|val| self.negated() ^ val)
  }
  /// Returns the variable for this literal as a usize
  /// for convenient indexing
  pub fn var(&self) -> usize { (self.0 >> 1) as usize }
  /// Returns what the var is assigned to if this lit is chosen.
  pub fn val(&self) -> bool { (self.0 & 1) == 0 }
  pub fn negated(&self) -> bool { (self.0 & 1) == 1 }
  pub fn is_negation(&self, o: &Self) -> bool { (self.0 ^ 1) == o.0 }
  /// Returns the raw internal of the literal
  // chose not to make this a usize because then it might take extra space on some machines
  // despite the fact that it's always a u32
  pub fn raw(&self) -> u32 { self.0 }
}

impl Not for Literal {
  type Output = Literal;
  fn not(self) -> Self::Output { Literal(self.0 ^ 1) }
}

impl Not for &'_ Literal {
  type Output = Literal;
  fn not(self) -> Self::Output { Literal(self.0 ^ 1) }
}

// Reads a literal from dimacs format
impl From<i32> for Literal {
  fn from(i: i32) -> Self {
    assert_ne!(i, 0);
    Literal::new((i.abs() as u32) - 1, i < 0)
  }
}

#[cfg(test)]
mod test {
  use super::*;
  #[test]
  pub fn test_new_literal() {
    let lit = Literal::from(-1);
    assert_eq!(lit.var(), 0);
    assert_eq!(lit.negated(), true);
    assert_eq!(lit.val(), false);
  }
}

impl Display for Literal {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}{}", if self.negated() { "!" } else { "" }, self.var())
  }
}
