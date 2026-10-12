---- MODULE Forms_C_function_helper_shadow ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 LOCAL F==1 G[F \in {1,2}]==F
 }
{ L:y:=G[1]; E:skip; }
}
*)
====
