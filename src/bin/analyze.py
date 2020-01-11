#!/usr/local/bin/python3

import numpy as np
from scipy import stats
import random
import matplotlib.pyplot as plt
import sys

if len(sys.argv) == 1:
  print("Pass file to analyze: $./analyze.py <CSV From Solver>")
  exit()
data = np.genfromtxt(sys.argv[1], delimiter=",", dtype=None, encoding=None)

# files solver was run on
files = {}

for row in data:
  files.setdefault(row[0], []).append(row.tolist()[1:-1])

# Mapping:
# Name, restarts, learned, propogs, tx->, tx<-, learnt_lits, time, #cores, sat
mappings = {0: "restarts", 1: "learned", "#cores": -1}
indeces = {y:x for x,y in mappings.items()}

np.set_printoptions(precision=3, suppress=True)
for k in files:
  v = np.array(files[k])
  desc = stats.describe(v)
  means = desc.mean
  stddevs = np.sqrt(desc.variance)
  yerr = 2*stddevs[indeces["learned"]]
  plt.errorbar(0, means[indeces["learned"]], yerr=yerr, fmt="-o")
  plt.show()
  exit()
