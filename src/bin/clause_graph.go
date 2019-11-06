/*
A simple binary to create a graphviz graph which relates clauses by the literals
they contain.
Can be run on a dimacs file by running `clause_graph -f <FILE>`.
*/
package main

import (
  "fmt"
  "os"
  "bufio"
  "flag"
  "log"
  "strings"
  "strconv"
  "sort"
)

var filePath = flag.String("f", "", "File to read graph from")

func abs(n int) int {
  if n > 0 {
    return n
  }
  return -n
}

func sign(n int) int {
  if n > 0 {
    return 1
  } else if n == 0 {
    return 0
  }
  return -1
}

func clauseString(c []int) string {
  var s = "("
  for i, lit := range c {
    if i == 0 {
      s += strconv.Itoa(lit)
      continue
    }
    s += fmt.Sprintf(", %d", lit)
  }
  s += ")"
  return s
}

func main() {
  flag.Parse()
  if *filePath == "" {
    log.Fatalln("Must pass file")
  }
  file, err := os.Open(*filePath)
  if err != nil {
    log.Fatalln(err)
  }
  var clauses [][]int
  var currClause []int
  scanner := bufio.NewScanner(file)
  for scanner.Scan() {
    t := scanner.Text()
    if strings.HasPrefix(t, "c") || strings.HasPrefix(t, "p") {
      continue
    }
    for _, part := range strings.Fields(t) {
      item, err := strconv.Atoi(part)
      if err != nil {
        log.Fatalln(err)
      }
      if item == 0 {
        clauses = append(clauses, currClause)
        currClause = nil
      } else {
        currClause = append(currClause, item)
      }
    }
  }
  var s strings.Builder
  s.WriteString("graph {\n")
  s.WriteString("  overlap = false;\n")
  // literal -> []idx in clauses
  literals := map[int][]int{}
  for i, clause := range clauses {
    sort.Ints(clause)
    for _, lit := range clause {
      literals[abs(lit)] = append(literals[abs(lit)], sign(lit) * i)
    }
  }
  for i, clause := range clauses {
    fmt.Fprintf(&s, "  %d [ label = \"%s\" ]\n", i, clauseString(clause))
  }
  for _, idxs := range literals {
    if len(idxs) == 1 {
      continue
    }
    for idx, i := range idxs {
      for _, j := range idxs[(idx+1):] {
        if sign(i) == sign(j) {
          fmt.Fprintf(&s, "  %d -- %d [ color=\"red\" ]\n", abs(i), abs(j))
          continue
        }
        fmt.Fprintf(&s, "  %d -- %d [ color=\"blue\" ]\n", abs(i), abs(j))
      }
    }
    s.WriteByte('\n')
  }


  s.WriteByte('}')
  fmt.Println(s.String())
}
