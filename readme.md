# small_sat

This is a SAT solver based on [MiniSAT](http://minisat.se/), which currently implements basic
CDCL solving.

It currently exposes 1 binaries, `solve_dimacs` which take as input a list
of [DIMACS](https://people.sc.fsu.edu/~jburkardt/data/cnf/cnf.html) files. `solve_dimacs` uses
CDCL solving with the data-structures and algorithms used in MiniSAT in order to more efficiently
find a so find a solution.

# Reproducing Results

In order to properly reproduce the results there are a couple of necessary dependencies:
[Rust](https://www.rust-lang.org/tools/install) with version
`rustc 1.42.0-nightly (859764425 2020-01-07)`. The dependencies will be installed at compilation
time.

In order to compile & run the executable on any CNF files:
```sh
$ cargo run --release <CNF files>
```

The benchmarks used for the results in the paper are in `$PROJECT_DIR/src/bin/data/bmc/`.
In order to produce the output files(requires [Ruby](https://www.ruby-lang.org/en/)), run
```sh
$ cd $PROJECT_DIR/src/bin
$ ./test_sound.rb
```
This may take some time, and I also manually edited `solve_dimacs.rs` to the number of desired
cores.

In order to generate the graphs, modify `analyze.py` in `$PROJECT_DIR/src/bin/` to use the
specified metric, and direct it to the set of output files from `test_sound.rb` or other CSV
files output by the solver.

In order to compile the paper, run the following commands:
```sh
pdflatex paper
bibtex paper
pdflatex paper
pdflatex paper
```

The source code is available in the `$PROJECT_DIR/src`
