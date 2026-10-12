---- MODULE RecursiveHeader_P_self_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Recur(n=0) variables v=n; begin R1:if n>0 then call Recur(n-1); else x:=x+v+1; end if;R2:return; end procedure;
begin L:call Recur(2); E:skip; end algorithm; *)
====
