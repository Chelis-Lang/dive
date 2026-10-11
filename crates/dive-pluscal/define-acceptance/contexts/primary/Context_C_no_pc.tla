---- MODULE Context_C_no_pc ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a) == a + 1
 }
{ L: while (TRUE) { x := F(1); }; }
}
*)
====
