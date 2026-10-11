---- MODULE Forms_C_function_init_conflict ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 Init[a \in {1,2}]==a
 }
{ L:y:=0; E:skip; }
}
*)
====
