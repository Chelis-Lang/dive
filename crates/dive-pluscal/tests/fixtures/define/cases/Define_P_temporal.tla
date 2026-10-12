---- MODULE Define_P_temporal ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F == [] (x=0)
 end define;
begin L: x := F; end algorithm;
*)
====
