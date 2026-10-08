---- MODULE PrintSets ----
EXTENDS Integers, Sequences, FiniteSets, TLC, TLCExt
CONSTANTS mvq1, mvq2, T_q1, T_q2, U_q1, mv_q3
ASSUME PrintT("#ints") /\ PrintT({3, 1, 2, -5, 2})
ASSUME PrintT("#strings") /\ PrintT({"qsc", "qsb", "qsa", "qsb"})
ASSUME PrintT("#bools") /\ PrintT({TRUE, FALSE})
ASSUME PrintT("#boolean") /\ PrintT(BOOLEAN)
ASSUME PrintT("#nested") /\ PrintT({{2}, {1}, {}, {1, 2}, {0, 5}})
ASSUME PrintT("#tuples") /\ PrintT({<<1, 2>>, <<1>>, <<>>, <<0, 9>>, <<2>>})
ASSUME PrintT("#records") /\ PrintT({[qfa |-> 2, qfb |-> 1], [qfa |-> 1, qfb |-> 2], [qfa |-> 1, qfb |-> 1]})
ASSUME PrintT("#mvints") /\ PrintT({5, mvq2, mvq1, 3})
ASSUME PrintT("#mvtyped") /\ PrintT({T_q2, mvq1, T_q1})
ASSUME PrintT("#intervals") /\ PrintT({1..3, 2..2, 5..4, {1, 2, 3}})
ASSUME PrintT("#intervalfirst") /\ PrintT({{1, 2, 3}, 1..3})
ASSUME PrintT("#setsofsets") /\ PrintT({{{1}}, {{}}, {{1}, {2}}})
ASSUME PrintT("#fcnmix") /\ PrintT({<<1>>, (1 :> 2), (2 :> 1)})
====
