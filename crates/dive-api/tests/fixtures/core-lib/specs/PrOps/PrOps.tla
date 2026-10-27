---- MODULE PrOps ----
\* TLC!Print and PrintT on the core (crates/dive-core/src/stdlib/tlc.rs):
\* start-up output first, then the search's in TLC's order. The effect bit
\* (crates/dive-core/src/compile.rs, `Info::effect`) keeps a constant print
\* from being folded, a definition that prints from being cached, and an
\* invariant conjunct that prints from being skipped.
EXTENDS Naturals, TLC
C == Print("const", 0)
ASSUME PrintT("assume")
VARIABLE x, y
Show(v) == PrintT(<<"show", v>>)
Twice(a) == a + a
Seen == PrintT(<<"seen", x>>)
Init == /\ PrintT("init")
        /\ x \in {0, 1}
        /\ PrintT(<<"init-x", x, {x, 5}, [a |-> x, b |-> {x}]>>)
        /\ y = {x}
Step(k) == /\ x' = (x + k) % 3
           /\ Show(x')
           /\ y' = {x'}
Next == \/ \E k \in {1, 2} : /\ PrintT(<<"next", x, k>>)
                             /\ Seen
                             /\ Seen
                             /\ Step(k)
        \/ /\ x = 2
           /\ LET v == Print(<<"let", x>>, x) IN x' = Twice(v) % 3
           /\ IF x = 2 THEN PrintT("two") ELSE TRUE
           /\ UNCHANGED y
Inv == /\ PrintT("inv-const")
       /\ PrintT(<<"inv", x>>)
       /\ x < 3
Constr == PrintT(<<"constr", x, y>>)
====
