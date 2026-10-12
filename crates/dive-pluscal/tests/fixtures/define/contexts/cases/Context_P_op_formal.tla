---- MODULE Context_P_op_formal ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(P(_)) == P(1)
 end define;
begin L: x := F(1); end algorithm;
*)
====
