pub fn luby(mut x: u64, y: u64) -> u64 {
  let mut size = 1;
  let mut seq = 0;
  while size < x + 1 {
    seq += 1;
    size = 2 * size + 1;
  }
  while size - 1 != x {
    size = (size - 1) >> 1;
    seq -= 1;
    x = x % size;
  }
  y.pow(seq)
}
