---- MODULE Forms_C_local_state ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 LOCAL F==x
 }
{ L:x:=1; y:=F; E:skip; }
}
*)
====
