---- MODULE Forms_C_local_function ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 LOCAL F[a \in {1,2}] == a+1
 }
{ L:y:=F[1]; E:skip; }
}
*)
====
