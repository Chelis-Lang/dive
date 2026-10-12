---- MODULE Forms_P_local_undefined ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 LOCAL F==Missing
 end define;
begin L:y:=F; E:skip;
end algorithm;
*)
====
