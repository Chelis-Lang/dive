---- MODULE Forms_C_local_undefined ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 LOCAL F==Missing
 }
{ L:y:=F; E:skip; }
}
*)
====
