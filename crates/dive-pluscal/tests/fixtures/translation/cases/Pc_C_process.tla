---- MODULE Pc_C_process ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--fair algorithm A { variable x = 0; fair + process (P \in {1, 2}) variable a = 3; { L: skip; } process (Q = 3) { M: skip; } }

*)
====
