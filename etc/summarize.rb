#!/usr/bin/env ruby

# A little script to determine the amount of time one symbol is in the call stack
# across many invocations of a program.

require 'optparse'
require 'ruby-progressbar'

options = {root: nil}
OptionParser.new do |opts|
  opts.banner = "Usage: summarize.rb [OPTIONS] QUERY PATH..."

  opts.on "-rROOT", "--root=ROOT", "Only consider call trees rooted in this symbol" do |root|
    options[:root] = root
  end
end.parse!

section = nil
prune_level = nil
matched_samples, total_samples = 0, 0

query = ARGV.shift

file_count = ARGV.length
progress_bar = ProgressBar.create(
  :total => file_count,
  :throttle_rate => 0.1,
  :format => '%t: |%B| %c/%C %p%% %e',
)

root_level = nil

while gets
  progress_bar.progress = file_count - ARGV.length - 1

  $_.chomp!

  (section = $1; next) if ~/^(\S[^:]*):$/
  next unless section == 'Call graph'

  next if not ~/^    ([ +!:|]*)(\d+) (\S+)/
  level = $1.length / 2
  samples = $2.to_i

  if options[:root].nil? && level == 0 or !options[:root].nil? && $3.include?(options[:root])
    total_samples += samples
    root_level = level
    next
  end

  if not root_level.nil? and level <= root_level
    root_level = nil
    next
  end

  next if not prune_level.nil? and level > prune_level
  prune_level = nil

  next unless $3.include? query
  matched_samples += samples
  prune_level = level
end

progress_bar.finish

puts "%.03f%%" % ((matched_samples.to_f / total_samples.to_f) * 100.0)
