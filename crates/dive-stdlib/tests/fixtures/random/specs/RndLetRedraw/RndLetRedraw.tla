---- MODULE RndLetRedraw ----
\* TLC surprise. Within one expression a LET-bound RandomElement is drawn once
\* (ASSUME, and the first PrintT of Init). Across the conjuncts of Init or of an
\* action, TLC draws it again at every conjunct that uses it, so the value
\* printed is not the value assigned to x.
EXTENDS Naturals, TLC
Twice(y) == <<y, y>>
ASSUME LET d == RandomElement(1..1000000) IN PrintT(<<"assume", d, d>>)
VARIABLE x
Init == LET d == RandomElement(1..1000000) IN
        /\ PrintT(<<"init", d, Twice(d)>>)
        /\ x = d
        /\ PrintT(<<"init-x", x, d>>)
Next == LET d == RandomElement(1..1000000) IN
        /\ PrintT(<<"next", x, d>>)
        /\ x' = IF x < 1000001 THEN 1000001 + d ELSE x
Inv == PrintT(<<"state", x>>)
====
