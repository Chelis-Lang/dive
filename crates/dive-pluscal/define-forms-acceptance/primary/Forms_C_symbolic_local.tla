---- MODULE Forms_C_symbolic_local ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 LOCAL a (+) b == a+b
 }
{ L:y:=1 (+) 2; E:skip; }
}
*)
====
