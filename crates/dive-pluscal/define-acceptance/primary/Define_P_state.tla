---- MODULE Define_P_state ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F == x
 end define;
begin L: x := 1; y := F; end algorithm;
*)
====
