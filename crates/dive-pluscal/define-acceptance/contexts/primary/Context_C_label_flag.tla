---- MODULE Context_C_label_flag ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a) == a + 1
 }
{ L: x := F(1); }
}
*)
====
