# small_sat

This is a SAT solver based on [MiniSAT](http://minisat.se/), which currently implements basic
CDCL solving.

It currently exposes 2 binaries, `dpll_solve` and `solve_dimacs`. They both take as input a list
of [DIMACS](https://people.sc.fsu.edu/~jburkardt/data/cnf/cnf.html) files. `dpll_solve` uses
dpll solving(Chronological Backtracking) in order to backtrack, while `solve_dimacs` uses CDCL
solving with the data-structures and algorithms used in MiniSAT in order to more efficiently
find a so find a solution.

`dpll_solve` was added to show correctness of basic structures, and only CDCL solving should be
used for any practical purpose.


