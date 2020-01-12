#!/usr/bin/ruby

100.times do |i|
  out = `cargo run --release data/bmc/bmc-*.cnf`
  File.open("#{i}.txt", "w") { |f| f.write(out) }
  if out.include? "UNSAT" then
    puts "UNSOUND"
    exit
  end
end
