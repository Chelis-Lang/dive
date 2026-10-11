---- MODULE Define_P_nested_comment ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F == (* note (* inner *) *) 1
 end define;
begin L: x := F; end algorithm;
*)
====
