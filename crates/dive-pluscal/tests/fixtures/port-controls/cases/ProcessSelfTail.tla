---- MODULE ProcessSelfTail ----
EXTENDS Integers, TLC
CONSTANT S
(*
--algorithm A variable x=0; procedure Recur(a=0,c=0) variable u=a+c; begin F: call Recur(a-1,a); return; end procedure; process Worker=1 begin L: call Recur(2,3); E: skip; end process; end algorithm
*)
====
