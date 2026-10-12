---- MODULE MarkerExtra_P_fair_malformed ----
EXTENDS Integers
(* --fairness algorithm A variable x = 0; begin L: x := 1; end algorithm;
--algorithm B { variable x = 0; { L: x := 2; } } *)
====
