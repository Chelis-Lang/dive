---- MODULE TsStored ----
\* ToString of a stored interval: TLC keeps 1..2 an interval in the state
\* and prints "1..2"; the core stores the set {1, 2} and so cannot know the
\* printed form: it refuses (crates/dive-core/src/print.rs, `ambiguous`).
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 1..2
Next == UNCHANGED x
Inv == ToString(x) = "1..2"
====
