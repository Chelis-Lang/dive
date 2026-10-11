---- MODULE Forms_P_symbolic_local ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 LOCAL a (+) b == a+b
 end define;
begin L:y:=1 (+) 2; E:skip;
end algorithm;
*)
====
