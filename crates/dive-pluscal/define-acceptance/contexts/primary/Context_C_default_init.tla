---- MODULE Context_C_default_init ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x, y=0;
 define {
 F(a) == a + 1
 }
{ L: x := F(1); }
}
*)
====
