---- MODULE Define_P_self ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F == self
 end define;
begin L: x := F; end algorithm;
*)
====
