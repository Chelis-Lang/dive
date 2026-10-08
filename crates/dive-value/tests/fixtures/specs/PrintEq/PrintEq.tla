---- MODULE PrintEq ----
EXTENDS Integers, Sequences, FiniteSets, TLC, TLCExt
CONSTANTS mvq1, mvq2, T_q1, T_q2, U_q1, mv_q3
ASSUME PrintT("#intv") /\ PrintT(<<1..3 = {1, 2, 3}, 3..1 = {}, 1..2 = 2..3>>)
ASSUME PrintT("#fcn") /\ PrintT(<<(1 :> 1 @@ 2 :> 2) = <<1, 2>>, <<>> = (1 :> 1), [qfa |-> 1] = ("qfa" :> 1)>>)
ASSUME PrintT("#mv") /\ PrintT(<<mvq1 = mvq1, mvq1 = mvq2, mvq1 = 1, mvq1 = "qsp", mvq1 = {}, T_q1 = mvq1, T_q1 = T_q2>>)
ASSUME PrintT("#lazy") /\ PrintT(<<SUBSET {1} = {{}, {1}}, [{1} -> {2}] = {<<2>>}, {1} \X {2} = {<<1, 2>>}, [qfa : {1}] = {[qfa |-> 1]}>>)
ASSUME PrintT("#emptyfcn") /\ PrintT(<<[{} -> {1}] = {<<>>}, <<>> = [qfa |-> 1]>>)
====
