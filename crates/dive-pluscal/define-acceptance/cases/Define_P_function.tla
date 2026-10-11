---- MODULE Define_P_function ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == [i \in 1..a |-> i]
 end define;
begin L: x := F(2); end algorithm;
*)
====
