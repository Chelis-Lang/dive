---- MODULE WhileTail_P_Test ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(z=0,a=0) begin P:x:=z+a; R:return; end procedure; begin L:while x=0 do skip; end while; if TRUE then call Put(1,2); goto E; else skip; end if; E:skip; end algorithm; *)
====
