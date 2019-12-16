#![feature(slice_partition_at_index)]
#![feature(div_duration)]
#![feature(weak_counts)]
mod clause;
pub mod database;
mod dimacs;
pub mod literal;
mod luby;
mod stats;
mod var_state;
mod watch_list;

pub mod solver;
