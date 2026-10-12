---- MODULE Context_C_initializer ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=F(1), y=0;
 define {
 F(a) == a + 1
 }
{ L: x := F(1); }
}
*)
====
