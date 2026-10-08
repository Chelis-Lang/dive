---- MODULE PrintScalars ----
EXTENDS Integers, Sequences, FiniteSets, TLC, TLCExt
CONSTANTS mvq1, mvq2, T_q1, T_q2, U_q1, mv_q3
ASSUME PrintT("#bools") /\ PrintT(<<TRUE, FALSE>>)
ASSUME PrintT("#ints") /\ PrintT(<<0, 7, -7, 2147483647, -2147483647 - 1>>)
ASSUME PrintT("#strings") /\ PrintT(<<"qsa", "", "q\"uote", "back\\slash", "tab\tx", "nl\nx", "cr\rx", "ff\fx">>)
ASSUME PrintT("#unicode") /\ PrintT("qé中")
ASSUME PrintT("#mvs") /\ PrintT(<<mvq1, mvq2, T_q1, U_q1>>)
ASSUME PrintT("#emptyset") /\ PrintT({})
ASSUME PrintT("#emptyseq") /\ PrintT(<<>>)
====
