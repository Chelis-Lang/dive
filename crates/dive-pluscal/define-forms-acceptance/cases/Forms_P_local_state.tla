---- MODULE Forms_P_local_state ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 LOCAL F==x
 end define;
begin L:x:=1; y:=F; E:skip;
end algorithm;
*)
====
