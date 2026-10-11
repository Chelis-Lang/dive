---- MODULE Forms_C_function_undefined_bound ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 F[a \in Missing]==a
 }
{ L:y:=0; E:skip; }
}
*)
====
