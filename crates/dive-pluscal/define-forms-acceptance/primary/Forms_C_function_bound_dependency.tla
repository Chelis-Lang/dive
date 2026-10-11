---- MODULE Forms_C_function_bound_dependency ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 LOCAL S=={1,2} F[a \in S]==a
 }
{ L:y:=F[1]; E:skip; }
}
*)
====
