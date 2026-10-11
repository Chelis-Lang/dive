---- MODULE Define_C_junction ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F == /\ x=0
     /\ y=0
 }
{ L: if (F) { x := 1; }; }
}
*)
====
