---- MODULE MultipleHeader_C_zero_args ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put() { P1:x:=3; P2:return; };procedure Get(q=0,b=0) { G1:x:=b+q; G2:return; }; { L:call Put(); M:call Get(3,4); E:skip; } } *)
====
