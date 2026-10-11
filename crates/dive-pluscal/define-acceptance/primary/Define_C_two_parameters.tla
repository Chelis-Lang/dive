---- MODULE Define_C_two_parameters ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a,b) == a+b
 }
{ L: x := F(1,2); }
}
*)
====
