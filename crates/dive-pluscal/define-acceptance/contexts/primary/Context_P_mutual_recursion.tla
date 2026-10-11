---- MODULE Context_P_mutual_recursion ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == G(a)
 G(a) == F(a)
 end define;
begin L: x := F(1); end algorithm;
*)
====
