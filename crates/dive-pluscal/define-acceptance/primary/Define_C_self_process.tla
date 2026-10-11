---- MODULE Define_C_self_process ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F == self
 }
 process (P \in {1,2})
 variables z=0;
 { L: x := F; }
}
*)
====
