---- MODULE Forms_C_function_empty ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 F[a \in {}]==a
 }
{ L:y:=Cardinality(DOMAIN F); E:skip; }
}
*)
====
