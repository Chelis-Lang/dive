---- MODULE PrRange ----
\* Print of an interval (as an argument: inside a tuple or set the core
\* enumerates it), and of values with no enumerated set of consecutive
\* integers, in a specification that builds intervals.
EXTENDS Naturals, TLC
VARIABLE x
Init == x \in 1..2
Next == /\ PrintT(<<"next", x, {x, x + 2}, [n |-> x], "s">>)
        /\ PrintT(1..x)
        /\ x' = 3 - x
Inv == Print(<<"inv", x>>, x \in 1..2)
====
