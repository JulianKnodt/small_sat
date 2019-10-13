import numpy as np

def dimacs_to_cmm(file_name):
  out = None
  num_vars = None
  curr_clause = 0
  with open(file_name) as f:
    for l in f.readlines():
      if l.startswith("c"):
        continue
      elif l.startswith("p cnf"):
        num_clauses, num_vars = map(int, l.split()[-2:])
        out = np.zeros((num_vars, num_clauses))
      else:
        for lit in map(int, l.split()):
          if lit is 0:
            curr_clause += 1
          else:
            out[curr_clause, abs(lit)-1] = np.sign(lit)
  return out

cmm = dimacs_to_cmm("data/aim-50-1_6-yes.cnf")
first_nonzero = np.argmax(cmm != 0, axis=1)
sorted_order = np.argsort(first_nonzero)
cmm = cmm[sorted_order]

def init_solver(clauses, numvars: int):
  clause_masking_matrix = np.zeros((len(clauses), numvars))
  for i, clause in enumerate(clauses):
    for variable in clause:
      assert(variable != 0)
      clause_masking_matrix[i, abs(variable)-1] = 1 if variable > 0 else -1
  # returns a clause masking matrix and an empty assignment matrix
  return clause_masking_matrix, np.zeros(numvars)

def allowed_falses(cmm):
  # returns the maximum number of allowed falses for a given clause before it becomes unsat
	return np.sum(np.abs(cmm), axis = 1)

def evaluate(cmm, curr_assn):
  # evaluates the current assignment given the clause masking matrix
  return cmm * curr_assn

def remaining_falses_per_row(cmm, curr_assn, num_allowed_falses=None):
  af = num_allowed_falses
  if af is None:
    af = allowed_falses(cmm)

  return af + np.matmul(cmm, curr_assn)

def unit_clauses(remaining_falses):
  return remaining_falses == 1

def unsat_clauses(remaining_falses):
  return remaining_falses == 0

def is_sat(cmm, curr_assn):
  return np.all(np.any(evaluate(cmm, curr_assn) == 1, axis=1))

OK = "ok"
UNSAT = "UNSAT"

# performs boolean constraint propogation for a given clause masking matrix
# and a given starting assignment.
# returns (assn, OK) or (conflict clause, UNSAT)
def bcp(cmm, curr_assn):
  rfpr = remaining_falses_per_row(cmm, curr_assn)
  unsat_clauses_mask = unsat_clauses(rfpr)
  if np.any(unsat_clauses_mask):
    return unsat_clauses_mask, UNSAT
  unit_clause_mask = unit_clauses(rfpr)
  if not np.any(unit_clause_mask):
    return curr_assn, OK # returns the current assignment, don't need to change anything
  unit_assns = (cmm[unit_clause_mask] + curr_assn) * np.abs(cmm[unit_clause_mask])
  maxs, mins = np.max(unit_assns, axis=0), np.min(unit_assn, axis=0)
  if not np.array_equal(maxs, mins):
    conflict_lits = (maxs+mins == 0)
    return conflict_lits, UNSAT
  result, status = bcp(cmm, curr_assns+np.sum(unit_assns, axis=0))
  if status is OK:
    return result, status
