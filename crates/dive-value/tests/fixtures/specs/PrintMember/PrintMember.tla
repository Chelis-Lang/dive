---- MODULE PrintMember ----
EXTENDS Integers, Sequences, FiniteSets, TLC, TLCExt
CONSTANTS mvq1, mvq2, T_q1, T_q2, U_q1, mv_q3
ASSUME PrintT("#intv") /\ PrintT(<<2 \in 1..3, 4 \in 1..3, mvq1 \in 1..3, "qsn" \in 3..1>>)
ASSUME PrintT("#nat") /\ PrintT(<<0 \in Nat, -1 \in Nat, -1 \in Int, mvq1 \in Nat>>)
ASSUME PrintT("#string") /\ PrintT(<<"qsn" \in STRING, mvq1 \in STRING>>)
ASSUME PrintT("#subset") /\ PrintT(<<{1} \in SUBSET {1, 2}, {3} \in SUBSET {1, 2}, {} \in SUBSET {}>>)
ASSUME PrintT("#fcnset") /\ PrintT(<<<<1, 2>> \in [{1, 2} -> {1, 2}], <<1>> \in [{1, 2} -> {1, 2}], (1 :> 2) \in [{1} -> {2}]>>)
ASSUME PrintT("#product") /\ PrintT(<<<<1, 2>> \in {1} \X {2}, <<1>> \in {1} \X {2}, <<1, 3>> \in {1} \X {2}>>)
ASSUME PrintT("#rcdset") /\ PrintT(<<[qfa |-> 1] \in [qfa : {1}], [qfa |-> 2] \in [qfa : {1}], [qfb |-> 1] \in [qfa : {1}]>>)
ASSUME PrintT("#enum") /\ PrintT(<<1 \in {1, "qso"}, mvq1 \in {1, 2}, 3 \in {1, 2}>>)
ASSUME PrintT("#seq") /\ PrintT(<<<<1, 1>> \in Seq({1}), <<>> \in Seq({1}), <<2>> \in Seq({1})>>)
====
