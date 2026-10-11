---- MODULE Forms_P_function_duplicate ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 F[a \in {1,2}]==a F[a \in {1,2}]==a+1
 end define;
begin L:y:=0; E:skip;
end algorithm;
*)
====
