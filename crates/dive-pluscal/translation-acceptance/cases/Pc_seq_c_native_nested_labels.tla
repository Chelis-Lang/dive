---- MODULE Pc_seq_c_native_nested_labels ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
{variable x=0;  L: while (x<2) { B: x:=x+1;} E: skip; }
*)
====
