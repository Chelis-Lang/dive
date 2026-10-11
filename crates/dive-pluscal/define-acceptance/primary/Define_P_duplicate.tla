---- MODULE Define_P_duplicate ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F == 1
 F == 2
 end define;
begin L: x := F; end algorithm;
*)
====
