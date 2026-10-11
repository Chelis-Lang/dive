---- MODULE Define_C_helper ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a) == a+1
 G == F(1)
 }
{ L: x := G; }
}
*)
====
