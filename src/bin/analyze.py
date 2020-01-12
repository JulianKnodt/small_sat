#!/usr/local/bin/python3

import numpy as np
import random
import matplotlib.pyplot as plt
import sys
import os

if len(sys.argv) == 1:
  print("Pass files to analyze: $./analyze.py [CSVs From Solver]")
  exit()

# files solver was run on
files = {}
for arg in sys.argv[1:]:
  data = np.genfromtxt(arg, delimiter=",", dtype=None, encoding=None)
  for row in data:
    assert(row[-1].strip() == "SAT")
    files.setdefault(row[0], []).append(row.tolist()[1:-1])

def from_nanos(nano) -> "Seconds": return nano / 1e9

# Mapping:
# Name, restarts, learned, propogs, tx->, tx<-, learnt_lits, time, #cores, sat
restarts = 0
learned = 1
imported = -4
cores = -1
time = -2

for filename in files:
  per_core = {}
  for row in files[filename]:
    per_core.setdefault(row[cores], []).append(row[learned])
  plt.boxplot(per_core.values())
  plt.ylabel("# Learned Clauses")
  plt.gca().axes.xaxis.set_ticklabels(per_core.keys())
  plt.xlabel("# Threads Used")
  display_name = os.path.basename(filename)
  plt.title(f'# Learned Clauses vs Thread Count for {display_name}')
  plt.show()
  # plt.savefig(os.path.splitext(display_name)[0])
  plt.clf()

