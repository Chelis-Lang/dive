---- MODULE Wide ----
\* Worker invariance: wide levels, several actions and a split action, and
\* `Print` in the next-state relation, invariants, constraints and implied
\* actions, each with `TLCGet("level")`. `Mode` selects which event, if any, stops the search part-way
\* through a level, so that every kind of stop is exercised.
EXTENDS Naturals, TLC
CONSTANT Mode
VARIABLES x, y

Guard == /\ y < 6
         /\ ~(Mode = "deadlock" /\ x = 37 /\ y = 3)

Init == x \in 0..20 /\ y = 0

A == /\ Guard
     /\ x' = (x * 7 + 3) % 50
     /\ y' = y + 1
     /\ IF x % 9 = 0 THEN PrintT(<<"A", x, y, TLCGet("level")>>) ELSE TRUE
B == /\ Guard
     /\ x' = (x + 11) % 50
     /\ y' = y + 1
     /\ (Mode = "error" /\ y = 3) => x' = 100 \div (x - 23)
C == \E k \in 1..3 : /\ Guard
                     /\ x' = (x + k * 5) % 50
                     /\ y' = y + 1
D == /\ Mode = "incomplete" /\ x = 31 /\ y = 4
     /\ x' = 0
Next == A \/ B \/ C \/ D

Inv == /\ IF x = 13 THEN PrintT(<<"inv", x, y, TLCGet("level")>>) ELSE TRUE
       /\ ~(Mode = "invariant" /\ x = 42 /\ y = 4)
       /\ ~(Mode = "constraint" /\ x = 44 /\ y = 4)
Constr == /\ IF x = 17 THEN PrintT(<<"constr", x, y, TLCGet("level")>>) ELSE TRUE
          /\ ~(Mode = "constraint" /\ x = 44 /\ y = 4)
Prop == [][/\ IF x' = 2 THEN PrintT(<<"prop", x, y, TLCGet("level")>>) ELSE TRUE
           /\ ~(Mode = "implied" /\ x = 29 /\ x' = 34 /\ y = 3)]_<<x, y>>
====
