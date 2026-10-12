---- MODULE Context_C_mutual_recursion ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a) == G(a)
 G(a) == F(a)
 }
{ L: x := F(1); }
}
*)
====
