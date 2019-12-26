#!/usr/local/bin/python3

import numpy as np
from scipy import stats
import random
import matplotlib.pyplot as plt
import sys

if len(sys.argv) == 1:
  print("Pass file to analyze")
  exit()
data = np.genfromtxt(sys.argv[1], delimiter=",", dtype=None, encoding=None)

# files solver was run on
files = {}

for row in data:
  files.setdefault(row[0], []).append(row.tolist()[1:-1])

for k in files:
  v = np.array(files[k])
  desc = stats.describe(v)
  means = desc.mean
  stddevs = np.sqrt(desc.variance)
  print(means)
