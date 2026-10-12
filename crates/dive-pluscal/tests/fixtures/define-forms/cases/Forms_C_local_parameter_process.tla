---- MODULE Forms_C_local_parameter_process ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 LOCAL F(a)==a+1
 }
process (P \in {1,2}) variable z=0; { L:y:=F(x); E:skip; }
}
*)
====
