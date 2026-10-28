---- MODULE RdShapes ----
\* RandomElement of each kind of set TLC draws from: an interval, an
\* enumeration, a record set and a function set (drawn per component),
\* SUBSET and a Cartesian product, and a LET-bound draw used twice in one
\* expression, which TLC draws once.
EXTENDS Naturals, TLC
VARIABLE x, n
Init == /\ x = RandomElement([a : 1..3, b : {"p", "q"}])
        /\ n = 0
Next == \/ /\ n < 5
           /\ n' = n + 1
           /\ x' = << RandomElement(SUBSET {"a", "b", "c"}),
                      RandomElement([1..2 -> 1..3]),
                      RandomElement({1, 2, 3} \X {"u", "v"}),
                      RandomElement({"s", "t", "w"}),
                      LET d == RandomElement(1..100) IN <<d, d>> >>
           /\ PrintT(<<n, x'>>)
        \/ /\ n = 5
           /\ UNCHANGED <<x, n>>
====
