#!/usr/bin/ruby

10.times do |i|
  out = `cargo run --release data/bmc/bmc-*.cnf`
  puts out
  if out.include? "UNSAT" then
    puts "UNSOUND"
    exit
  end
end
