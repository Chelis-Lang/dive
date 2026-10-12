---- MODULE RecursiveTail_P_mutual_tail_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Recur(n=0) variables v=n; begin R1:if n>0 then call Other(n-1);return; else x:=x+1; end if;R2:return; end procedure;
procedure Other(m=0) variables q=m; begin O1:if m>0 then call Recur(m-1);return; else x:=x+1; end if;O2:return; end procedure;
begin L:call Recur(3); E:skip; end algorithm; *)
====
