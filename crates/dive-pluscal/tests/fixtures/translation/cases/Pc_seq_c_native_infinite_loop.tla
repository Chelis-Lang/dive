---- MODULE Pc_seq_c_native_infinite_loop ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
{variable x=0;  L: while (TRUE) {x:=(x+1)%2;} }
*)
====
