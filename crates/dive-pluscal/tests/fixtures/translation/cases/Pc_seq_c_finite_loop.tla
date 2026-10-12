---- MODULE Pc_seq_c_finite_loop ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; { L: while (x<2) {x:=x+1;} E: skip; }
*)
====
