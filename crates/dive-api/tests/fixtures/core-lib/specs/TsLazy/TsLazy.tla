---- MODULE TsLazy ----
\* ToString of intervals and lazy sets, which TLC prints by their form:
\* the core keeps them symbolic (crates/dive-core/src/print.rs). Inside a
\* tuple or set the core enumerates them, and printing is refused.
EXTENDS Naturals, TLC
VARIABLE s
Init == s = ToString(1..2)
Next == UNCHANGED s
Inv == /\ s = "1..2"
       /\ ToString(1..3) = "1..3"
       /\ ToString(3..1) = "{}"
       /\ ToString(SUBSET (1..7)) = "SUBSET (1..7)"
       /\ ToString(SUBSET {"a"}) = "{{}, {\"a\"}}"
       /\ ToString([{"a", "b"} -> {"c"}]) = "{[a |-> \"c\", b |-> \"c\"]}"
       /\ ToString([a : {1, 3}]) = "{[a |-> 1], [a |-> 3]}"
       /\ ToString({"p"} \X {3, 5}) = "{<<\"p\", 3>>, <<\"p\", 5>>}"
====
