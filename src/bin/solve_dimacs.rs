use small_sat::{literal::Literal, solver::Solver};
use std::env;

fn output(assns: Vec<bool>) -> String {
  assns
    .iter()
    .enumerate()
    .map(|(i, &val)| format!("{}", Literal::new((i + 1) as u32, val)))
    .collect::<Vec<_>>()
    .join(" & ")
}

fn main() {
  for arg in env::args().skip(1).filter(|v| !v.starts_with("--")) {
    println!("Reading from: {}", arg);
    let mut solver = Solver::from_dimacs(arg).expect("Failed to create solver from dimacs");
    let out = solver.solve();
    let output = match out {
      None => String::from("UNSAT"),
      Some(sol) => format!("SAT ({})", output(sol)),
    };
    println!("{}", output);
  }
}
