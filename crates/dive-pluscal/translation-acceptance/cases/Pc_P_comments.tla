---- MODULE Pc_P_comments ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A begin x := <<1, (* nested (* comment *) okay *) 2>>; \* ending
 print "end; begin \""; end algorithm

*)
====
