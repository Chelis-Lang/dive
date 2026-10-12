---- MODULE Context_C_macro ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a) == a + 1
 }
macro M(a) { x := F(a); };
{ L: M(1); }
}
*)
====
