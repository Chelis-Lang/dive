---- MODULE Forms_P_function_recursive ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 F[a \in {1,2}]==IF a=1 THEN 0 ELSE F[a-1]
 end define;
begin L:y:=F[2]; E:skip;
end algorithm;
*)
====
