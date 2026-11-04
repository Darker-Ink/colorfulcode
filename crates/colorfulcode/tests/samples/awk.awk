#!/usr/bin/awk -f
# Summarise request latency per endpoint from an access log.

BEGIN {
    FS = " "
    OFS = "\t"
    threshold = 0.5
    print "endpoint", "count", "avg", "slow"
}

function round(x, places,    m) {
    m = 10 ^ places
    return int(x * m + 0.5) / m
}

$9 ~ /^5[0-9][0-9]$/ { errors++ }

/GET|POST/ && NF > 10 {
    path = $7
    sub(/\?.*$/, "", path)
    latency = $NF + 0
    count[path]++
    total[path] += latency
    if (latency > threshold) slow[path]++
}

END {
    for (p in count) {
        avg = total[p] / count[p]
        printf "%-30s %5d %8.3f %d\n", p, count[p], round(avg, 3), slow[p]
    }
    if (errors > 0) print "server errors: " errors > "/dev/stderr"
    printf("processed %d lines from %s\n", NR, FILENAME)
}
