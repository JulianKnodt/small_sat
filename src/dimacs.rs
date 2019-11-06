use crate::{clause::Clause, literal::Literal};
use std::io;

pub fn from_dimacs<S>(s: S) -> io::Result<(Vec<Clause>, usize)>
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
  Ok((clauses, max_var))
}