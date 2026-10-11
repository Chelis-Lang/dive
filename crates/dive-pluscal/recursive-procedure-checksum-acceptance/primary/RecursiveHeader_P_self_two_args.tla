---- MODULE RecursiveHeader_P_self_two_args ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Recur(n=0,k=0) begin R1:if n>0 then call Recur(n-1,k+1); else x:=k; end if;R2:return; end procedure;
begin L:call Recur(2,0); E:skip; end algorithm; *)
====
