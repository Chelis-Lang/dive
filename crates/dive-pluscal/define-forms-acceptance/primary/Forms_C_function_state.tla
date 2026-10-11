---- MODULE Forms_C_function_state ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 F[a \in {1,2}] == x+a
 }
{ L:x:=1; y:=F[1]; E:skip; }
}
*)
====
