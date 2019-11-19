use small_sat::{literal::Literal, solver::Solver};
use std::{env, thread};

fn output(assns: &Vec<bool>) -> String {
  assns
    .iter()
    .enumerate()
    .map(|(i, &val)| format!("{}", Literal::new((i + 1) as u32, !val)))
    .collect::<Vec<_>>()
    .join(" & ")
}

fn main() {
  // specify how many cores to run this on
  let num_cores = 1;
  for arg in env::args().skip(1).filter(|v| !v.starts_with("--")) {
    println!("Reading from: {}", arg);
    let solver = Solver::from_dimacs(arg).expect("Failed to create solver from dimacs");
    let handles = (0..num_cores).map(move |_| {
      let mut solver = solver.clone();
      thread::spawn(move || {
        match solver.solve() {
          None => println!("UNSAT"),
          Some(sol) => {
            println!("SAT ({})", output(&sol));
            let ok = solver.db.initial_clauses.iter().all(|c| c.is_sat(&sol));
            assert!(ok);
          },
        };
      })
    });
    for handle in handles {
      handle.join().expect("Thread panicked");
    }
  }
}
