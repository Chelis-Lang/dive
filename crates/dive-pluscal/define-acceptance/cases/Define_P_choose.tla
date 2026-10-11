---- MODULE Define_P_choose ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == CHOOSE i \in 1..a : TRUE
 end define;
begin L: x := F(2); end algorithm;
*)
====
