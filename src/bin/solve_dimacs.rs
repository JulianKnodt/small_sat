extern crate core_affinity;

use small_sat::solver::Solver;
use std::{env, thread, time::Instant};

fn main() {
  // specify how many cores to run this on
  let start = Instant::now();
  for arg in env::args().skip(1).filter(|v| !v.starts_with("--")) {
    println!("Starting {:?}", arg);
    multi_threaded(&arg);
    // single_threaded(&arg);
  }
  println!("Total elapsed for all: {:?}", start.elapsed());
}

#[allow(dead_code)]
fn single_threaded(s: &'_ str) {
  let mut solver = Solver::from_dimacs(s).expect("Could not open dimacs file");
  let result = solver.solve();
  match result {
    None => println!("{} UNSAT", s),
    Some(sol) => {
      assert!(solver.db.initial_clauses.iter().all(|c| c.is_sat(&sol)));
      println!("{} SAT", s);
    },
  };
}

#[allow(dead_code)]
fn multi_threaded(s: &'_ str) {
  use std::sync::mpsc::channel;
  let core_ids = core_affinity::get_core_ids().expect("Could not get core ids");
  let mut solvers = Solver::from_dimacs(s)
    .expect("Could not open dimacs file")
    .replicate(core_ids.len())
    .expect("Failed to replicate solver");
  let initials = solvers[0].db.initial_clauses.clone();

  let (sender, receiver) = channel();
  let n = core_ids.into_iter().map(move |id| {
    let mut solver = solvers.pop().unwrap();
    let sender = sender.clone();
    thread::spawn(move || {
      core_affinity::set_for_current(id);
      // Safe to ignore error here because only care about first that finishes
      let result = solver.solve();
      let _ = sender.send(result);
    });
  }).count();

  match receiver.recv().unwrap() {
    None => println!("{} UNSAT", s),
    Some(sol) => {
      assert!(initials.iter().all(|clause| clause.is_sat(&sol)));
      println!("{} SAT", s);
    },
  };
  for _ in 0..(n-1) {
    receiver.recv().expect("Failed to receive");
  }
}
