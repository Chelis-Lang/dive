---- MODULE Pc_seq_c_current_elsif ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
{variable x=0; { L: if (x=0) {x:=1;} else if (x=1) {x:=2;} else {x:=3;} E: skip; }}
*)
====
