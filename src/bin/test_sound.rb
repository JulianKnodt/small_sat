#!/usr/bin/ruby

100.times do |i|
  out = `RUST_BACKTRACE=1 cargo run --release data/bmc/bmc-*.cnf 2>&1 > #{i}.txt`
  if out.include? "UNSAT" then
    puts "UNSOUND"
    exit
  end
end
