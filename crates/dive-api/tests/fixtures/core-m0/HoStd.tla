---- MODULE HoStd ----
\* Standard operators passed to higher-order operators, and
\* DyadicRationals!Reduce (issue #849).
EXTENDS Naturals, Sequences, Functions, FiniteSetsExt, SequencesExt, DyadicRationals
VARIABLE x
f == [i \in 1..3 |-> i * x]
Init == x = 0
Next == x' = (x + 1) % 5
Sum == FoldFunctionOnSet(+, 0, f, {1, 2})
Cat == FoldSeq(\o, <<>>, <<<<x>>, <<1>>>>)
Prod == FoldSet( * , 1, {x + 1, 2})
Half3 == Half(Half([num |-> x, den |-> 1]))
Inv == /\ Sum = FoldFunctionOnSet(LAMBDA a, b : a + b, 0, f, {1, 2})
       /\ Cat = FoldSeq(LAMBDA a, b : a \o b, <<>>, <<<<x>>, <<1>>>>)
       /\ Prod = FoldSet(LAMBDA a, b : a * b, 1, {x + 1, 2})
       /\ Add(Half3, Half3) = Half(Half([num |-> 2 * x, den |-> 1]))
====
