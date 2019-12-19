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
  let mut max_seen_var = 0;
  for line in buf_reader.lines() {
    let line = line?;
    let line = line.trim();
    if line.starts_with('c') {
      continue;
    }
    if line.starts_with("p cnf") {
      let mut items = line
        .split_whitespace()
        .filter_map(|v| v.parse::<usize>().ok());
      max_var = items.next().expect("Missing # variables from \"p cnf\"");
      clauses.reserve(items.next().expect("Missing # clauses from \"p cnf\""));
    } else {
      line
        .split_whitespace()
        .map(|v| {
          v.parse::<i32>()
            .expect("Failed to parse int in dimacs file")
        })
        .for_each(|v| match v {
          0 => {
            curr_lits.shrink_to_fit();
            let mut complete_clause =
              Clause::from(mem::replace(&mut curr_lits, Vec::with_capacity(3)));
            complete_clause.initial = true;
            clauses.push(complete_clause);
          },
          v => {
            let lit = Literal::from(v);
            max_seen_var = max_seen_var.max(lit.var() + 1);
            curr_lits.push(lit);
          },
        });
    }
  }
  assert_eq!(
    max_seen_var, max_var,
    "DIMAC's file max variable incorrect got {}, expected {}",
    max_seen_var, max_var
  );
  clauses.shrink_to_fit();
  Ok((clauses, max_var))
}
