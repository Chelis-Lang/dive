---- MODULE RecursiveHeader_P_nonterminating_self ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Recur(n=0) begin R1:call Recur(n); R2:return; end procedure;
begin L:call Recur(2); E:skip; end algorithm; *)
====
