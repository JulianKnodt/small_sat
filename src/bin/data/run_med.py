#!/usr/bin/env python3

import os

meds = os.listdir("med")
for med in meds:
  out = os.popen("minisat med/%s" % med)
  out.read()
