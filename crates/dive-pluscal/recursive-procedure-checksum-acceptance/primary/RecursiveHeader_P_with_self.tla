---- MODULE RecursiveHeader_P_with_self ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Recur(n=0) begin R1:if n>0 then with w=n-1 do call Recur(w); end with; else x:=x+1; end if;R2:return; end procedure;
begin L:call Recur(2); E:skip; end algorithm; *)
====
