---- MODULE RdViol ----
\* A violation reached through draws: TLC rebuilds the trace by drawing
\* again from each predecessor's stream.
EXTENDS Naturals, TLC
VARIABLE x
Init == x = RandomElement(0..2)
Next == \/ x' = x + RandomElement(1..5)
        \/ x' = x + RandomElement({2, 4})
Inv == x < 12
====
