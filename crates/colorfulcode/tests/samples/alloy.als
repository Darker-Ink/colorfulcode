// A simple file system model
module filesystem

open util/ordering[Time] as T

sig Time {}

abstract sig Object {}
sig File extends Object {}
sig Dir extends Object {
    contents: set Object -> Time,
    parent: lone Dir
}
one sig Root extends Dir {}

-- every object except the root has exactly one parent
fact NoOrphans {
    all o: Object - Root | one d: Dir | o in d.contents.Time
    no Root.parent
}

fact Acyclic {
    no d: Dir | d in d.^parent
}

pred add[d: Dir, o: Object, t, t2: Time] {
    o not in d.contents.t
    d.contents.t2 = d.contents.t + o
}

fun descendants[d: Dir]: set Object {
    d.^(contents.Time)
}

/* the root reaches everything */
assert RootReachesAll {
    all o: Object - Root | o in descendants[Root]
}

check RootReachesAll for 5 but exactly 3 Time
run add for 4 Object, 2 Time
