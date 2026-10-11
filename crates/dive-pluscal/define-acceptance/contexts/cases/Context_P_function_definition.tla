---- MODULE Context_P_function_definition ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F[a \in {1,2}] == a+1
 end define;
begin L: x := F[1]; end algorithm;
*)
====
