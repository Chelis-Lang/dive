---- MODULE Define_P_escaped ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F == "a\n"
 end define;
begin L: x := F; end algorithm;
*)
====
