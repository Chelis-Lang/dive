---- MODULE Forms_C_local_chain ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 LOCAL G(a)==a+1 LOCAL F(a)==G(a)
 }
{ L:y:=F(x); E:skip; }
}
*)
====
