---- MODULE Context_C_multi_helper ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a) == a + 1
 G(a) == F(a) + 1
 }
{ L: x := G(1); }
}
*)
====
