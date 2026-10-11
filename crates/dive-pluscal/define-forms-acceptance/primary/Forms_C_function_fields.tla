---- MODULE Forms_C_function_fields ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 LOCAL G==1 F[a \in {1,2}]==[G |-> a].G
 }
{ L:y:=F[1]; E:skip; }
}
*)
====
