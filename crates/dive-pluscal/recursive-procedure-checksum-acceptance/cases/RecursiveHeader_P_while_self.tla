---- MODULE RecursiveHeader_P_while_self ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Recur(n=0) begin R1:while n>0 do WC:call Recur(n-1); WA:n:=0; end while;R2:return; end procedure;
begin L:call Recur(2); E:skip; end algorithm; *)
====
