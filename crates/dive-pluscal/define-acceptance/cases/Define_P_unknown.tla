---- MODULE Define_P_unknown ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F == Unknown
 end define;
begin L: x := F; end algorithm;
*)
====
