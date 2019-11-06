use small_sat::solver::Solver;
use std::env;

fn main() {
  for arg in env::args().skip(1).filter(|v| !v.starts_with("--")) {
    println!("Reading from: {}", arg);
    let mut solver = Solver::from_dimacs(arg).expect("Failed to create solver from dimacs");
    println!("{:?}", solver);
    let out = solver.dpll_solve();
    println!("{:?}", out);
  }
}
