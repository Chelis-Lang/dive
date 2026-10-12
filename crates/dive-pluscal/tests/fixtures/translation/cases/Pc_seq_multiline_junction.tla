---- MODULE Pc_seq_multiline_junction ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: print /\ TRUE
                            /\ x=0; E: skip; end algorithm;
*)
====
