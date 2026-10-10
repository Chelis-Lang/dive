---- MODULE Pc_seq_c_complete_if ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
{variable x=0; { L: if (x=0) {x:=1;} else {x:=2;}; E: skip; }}
*)
====
