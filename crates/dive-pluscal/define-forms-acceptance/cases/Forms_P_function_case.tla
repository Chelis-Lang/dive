---- MODULE Forms_P_function_case ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 F[a \in {1,2}]==CASE a=1 -> 2 [] OTHER -> 0
 end define;
begin L:y:=F[1]; E:skip;
end algorithm;
*)
====
