use std::{
  fmt::{self, Debug, Display},
  hash::Hash,
  ops::Not,
};

// Defines a literal
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Literal(u32);

impl Literal {
  #[inline]
  pub const fn new(var: u32, negated: bool) -> Self { Self((var << 1) + (negated as u32)) }
  // Panics if this variable is not in the vector
  /// returns the value for this literal given these assignments
  #[inline]
  pub fn assn(&self, assignments: &Vec<Option<bool>>) -> Option<bool> {
    assignments[self.var()].map(|val| self.negated() ^ val)
  }
  /// Returns the variable for this literal as a usize
  /// for convenient indexing
  #[inline]
  pub const fn var(&self) -> usize { (self.0 >> 1) as usize }
  /// Returns what the var is assigned to if this lit is chosen.
  #[inline]
  pub const fn val(&self) -> bool { (self.0 & 1) == 0 }
  #[inline]
  pub const fn negated(&self) -> bool { (self.0 & 1) == 1 }
  pub const fn is_negation(&self, o: &Self) -> bool { (self.0 ^ 1) == o.0 }
  /// Returns the raw internal of the literal
  // chose not to make this a usize because then it might take extra space on some machines
  // despite the fact that it's always a u32, even though it is only used as an index
  #[inline]
  pub const fn raw(&self) -> u32 { self.0 }
}

impl Not for Literal {
  type Output = Literal;
  #[inline]
  fn not(self) -> Self::Output { Literal(self.0 ^ 1) }
}

impl Not for &'_ Literal {
  type Output = Literal;
  #[inline]
  fn not(self) -> Self::Output { Literal(self.0 ^ 1) }
}

// Reads a literal from dimacs format
impl From<i32> for Literal {
  #[inline]
  fn from(i: i32) -> Self {
    debug_assert_ne!(i, 0);
    Literal::new((i.abs() as u32) - 1, i < 0)
  }
}

impl From<u32> for Literal {
  #[inline]
  fn from(u: u32) -> Self { Literal(u) }
}

#[cfg(test)]
mod test {
  use super::*;
  #[test]
  pub fn test_new_literal() {
    (1..42i32).for_each(|var| {
      let lit = Literal::from(-var);
      assert_eq!(lit.var(), (var - 1) as usize);
      assert!(lit.negated());
      assert_eq!(lit.val(), false);
      assert_eq!((!lit).var(), (var - 1) as usize);
      assert!(!(!lit).negated());
      assert_eq!((!lit).val(), true);
    });
  }
}

impl Display for Literal {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}{}", if self.negated() { "!" } else { "" }, self.var())
  }
}
