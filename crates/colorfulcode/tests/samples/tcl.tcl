#!/usr/bin/env tclsh
# Word frequency counter
package require Tcl 8.6

namespace eval ::wordfreq {
    variable counts [dict create]
}

proc ::wordfreq::add {text} {
    variable counts
    foreach word [split [string tolower $text] " "] {
        if {$word eq ""} { continue }
        dict incr counts $word
    }
    return [dict size $counts]
}

proc report {{limit 10}} {
    set items [lsort -stride 2 -index 1 -integer -decreasing $::wordfreq::counts]
    set i 0
    foreach {word n} $items {
        puts [format "%-12s %5d" $word $n]
        if {[incr i] >= $limit} break
    }
}

set data "the cat and the hat\tsat"
::wordfreq::add $data
set total [expr {0x10 + 2.5 * 3}]
puts "Total: ${total} words, first is [lindex $data 0]"
array set colors {red #ff0000 green #00ff00}
if {[catch {open missing.txt r} err]} {
    puts stderr "Error: $err"
} else {
    report 5
}
