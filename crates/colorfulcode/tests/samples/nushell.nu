# Summarise the largest files in a directory.
use std log

const DEFAULT_LIMIT = 10

# Show the biggest files below `path`.
def biggest [
    path: path = .        # where to look
    --limit (-l): int = 10
    --json
] {
    let files = (ls **/* | where type == file | sort-by size --reverse)
    let top = ($files | first $limit)
    if $json {
        $top | to json
    } else {
        $top | select name size modified
    }
}

export def --env goto [name: string] {
    let target = ($env.HOME | path join "code" $name)
    cd $target
    print $"now in ($target) with (ls | length) entries"
}

mut total = 0
for row in (open data.csv) {
    $total += $row.amount * 1.5
}

let config = {name: "colorful", tags: [cli rust], size: 10kb, wait: 5sec}
let raw = r#'C:\path\no\escapes'#
match $config.name {
    "colorful" => { log info $'matched ($config.name)' }
    _ => { error make {msg: "unknown project"} }
}
1..5 | each {|i| $i * 2 } | math sum
$env.PATH = ($env.PATH | prepend '/opt/bin')
