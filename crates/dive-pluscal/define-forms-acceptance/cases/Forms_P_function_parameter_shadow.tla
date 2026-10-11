---- MODULE Forms_P_function_parameter_shadow ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 F[x \in {1,2}]==x+1
 end define;
begin L:y:=F[1]; E:skip;
end algorithm;
*)
====
