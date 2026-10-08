---- MODULE PrintFcns ----
EXTENDS Integers, Sequences, FiniteSets, TLC, TLCExt
CONSTANTS mvq1, mvq2, T_q1, T_q2, U_q1, mv_q3
ASSUME PrintT("#tuple") /\ PrintT(<<1, "qsd", {2, 1}, <<>>>>)
ASSUME PrintT("#record") /\ PrintT([qfb |-> 1, qfa |-> {1}, qfc |-> <<2>>])
ASSUME PrintT("#fcnint") /\ PrintT((1 :> 2 @@ 3 :> 4))
ASSUME PrintT("#fcntuple") /\ PrintT((2 :> "qse" @@ 1 :> "qsf"))
ASSUME PrintT("#fcnrcd") /\ PrintT(("qsg" :> 1 @@ "qsh" :> 2))
ASSUME PrintT("#fcnspace") /\ PrintT(("q s" :> 1))
ASSUME PrintT("#fcnset") /\ PrintT(({1} :> 2 @@ {} :> 3))
ASSUME PrintT("#fcnmv") /\ PrintT((mvq1 :> 1 @@ mvq2 :> 2))
ASSUME PrintT("#fcnneg") /\ PrintT((-1 :> 1 @@ 0 :> 2))
ASSUME PrintT("#fcnfrom2") /\ PrintT((2 :> 1 @@ 3 :> 2))
ASSUME PrintT("#nestedrec") /\ PrintT([qfa |-> [qfb |-> <<1, 2>>], qfc |-> {[qfa |-> 1]}])
====
