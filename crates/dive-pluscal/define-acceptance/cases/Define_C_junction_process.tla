---- MODULE Define_C_junction_process ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F == /\ x=0
     /\ y=0
 }
 process (P \in {1,2})
 variables z=0;
 { L: if (F) { x := 1; }; }
}
*)
====
