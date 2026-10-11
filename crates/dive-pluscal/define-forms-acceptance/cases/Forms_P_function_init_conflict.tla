---- MODULE Forms_P_function_init_conflict ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 Init[a \in {1,2}]==a
 end define;
begin L:y:=0; E:skip;
end algorithm;
*)
====
