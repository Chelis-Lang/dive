---- MODULE Define_C_state_process ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F == x
 }
 process (P \in {1,2})
 variables z=0;
 { L: x := 1; y := F; }
}
*)
====
