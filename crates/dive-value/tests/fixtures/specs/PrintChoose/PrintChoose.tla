---- MODULE PrintChoose ----
EXTENDS Integers, Sequences, FiniteSets, TLC, TLCExt
CONSTANTS mvq1, mvq2, T_q1, T_q2, U_q1, mv_q3
ASSUME PrintT("#ints") /\ PrintT(CHOOSE x \in {3, 1, 2} : TRUE)
ASSUME PrintT("#strings") /\ PrintT(CHOOSE x \in {"qsk", "qsj"} : TRUE)
ASSUME PrintT("#interval") /\ PrintT(CHOOSE x \in 4..9 : TRUE)
ASSUME PrintT("#subset") /\ PrintT(CHOOSE x \in SUBSET {1, 2} : TRUE)
ASSUME PrintT("#fcnset") /\ PrintT(CHOOSE x \in [{1, 2} -> {8, 7}] : TRUE)
ASSUME PrintT("#product") /\ PrintT(CHOOSE x \in {2, 1} \X {4, 3} : TRUE)
ASSUME PrintT("#rcdset") /\ PrintT(CHOOSE x \in [qfb : {2, 1}, qfa : {"qsl", "qsm"}] : TRUE)
ASSUME PrintT("#mv") /\ PrintT(CHOOSE x \in {5, mvq2, mvq1} : TRUE)
ASSUME PrintT("#sets") /\ PrintT(CHOOSE x \in {{1, 2}, {3}, {0, 9}} : TRUE)
ASSUME PrintT("#tuples") /\ PrintT(CHOOSE x \in {<<2>>, <<1, 1>>, <<3>>} : TRUE)
ASSUME PrintT("#bools") /\ PrintT(CHOOSE x \in BOOLEAN : TRUE)
ASSUME PrintT("#recs") /\ PrintT(CHOOSE x \in {[qfa |-> 2], [qfb |-> 1]} : TRUE)
====
