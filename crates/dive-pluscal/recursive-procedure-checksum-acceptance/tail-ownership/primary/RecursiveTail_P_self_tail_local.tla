---- MODULE RecursiveTail_P_self_tail_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Recur(n=0) variables v=n; begin R1:if n>0 then call Recur(n-1); return; else x:=x+v+1; return; end if; end procedure;
begin L:call Recur(2); E:skip; end algorithm; *)
====
