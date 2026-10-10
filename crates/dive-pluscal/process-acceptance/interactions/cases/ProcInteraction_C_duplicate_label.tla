---- MODULE ProcInteraction_C_duplicate_label ----
EXTENDS Naturals,TLC
(* --algorithm A { process (P \in{1,2}) { L: skip; L: skip; } } *)
====
