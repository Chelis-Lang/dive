---- MODULE Forms_P_function_bound_dependency ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 LOCAL S=={1,2} F[a \in S]==a
 end define;
begin L:y:=F[1]; E:skip;
end algorithm;
*)
====
