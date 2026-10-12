---- MODULE RecursiveHeader_P_self_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Recur(n=0) begin R1:if n>0 then call Recur(n-1); return; else x:=x+1; return; end if; end procedure;
begin L:call Recur(2); E:skip; end algorithm; *)
====
