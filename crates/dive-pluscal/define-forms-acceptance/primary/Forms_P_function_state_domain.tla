---- MODULE Forms_P_function_state_domain ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 F[a \in 0..x] == a+1
 end define;
begin L:y:=F[0]; E:skip;
end algorithm;
*)
====
