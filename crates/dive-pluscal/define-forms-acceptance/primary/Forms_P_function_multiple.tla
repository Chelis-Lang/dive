---- MODULE Forms_P_function_multiple ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 F[a \in {1,2}, b \in {2,3}] == a+b
 end define;
begin L:y:=F[1,2]; E:skip;
end algorithm;
*)
====
