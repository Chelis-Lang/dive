---- MODULE Forms_P_function_calls_local ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 LOCAL G(a)==a+1 F[a \in {1,2}]==G(a)
 end define;
begin L:y:=F[1]; E:skip;
end algorithm;
*)
====
