---- MODULE RecursiveHeader_P_mutual_reverse ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Other(m=0) begin O1:if m>0 then call Recur(m-1); else x:=x+1; end if;O2:return; end procedure;
procedure Recur(n=0) begin R1:if n>0 then call Other(n-1); else x:=x+1; end if;R2:return; end procedure;
begin L:call Recur(3); E:skip; end algorithm; *)
====
