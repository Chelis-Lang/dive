---- MODULE RecursiveHeader_P_elsif_self ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Recur(n=0) begin R1:if n=0 then x:=1; elsif n>0 then call Recur(n-1); else x:=0; end if;R2:return; end procedure;
begin L:call Recur(2); E:skip; end algorithm; *)
====
