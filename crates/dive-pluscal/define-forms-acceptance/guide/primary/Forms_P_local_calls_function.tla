---- MODULE Forms_P_local_calls_function ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 F[a \in {1,2}]==a+1 LOCAL G(a)==F[a]
 end define;
begin L:y:=G(1); E:skip;
end algorithm;
*)
====
