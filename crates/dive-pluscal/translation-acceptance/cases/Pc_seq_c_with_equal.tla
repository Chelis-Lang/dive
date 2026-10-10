---- MODULE Pc_seq_c_with_equal ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; { L: with (k=2) {x:=k;} E: skip; }
*)
====
