---- MODULE Context_C_fair_marker ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--fair algorithm A {
 variables x=0, y=0;
 define {
 F(a) == a + 1
 }
{ L: x := F(1); }
}
*)
====
