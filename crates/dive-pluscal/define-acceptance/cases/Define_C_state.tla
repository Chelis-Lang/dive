---- MODULE Define_C_state ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F == x
 }
{ L: x := 1; y := F; }
}
*)
====
