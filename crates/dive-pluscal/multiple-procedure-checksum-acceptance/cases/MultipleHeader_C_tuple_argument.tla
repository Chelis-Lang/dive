---- MODULE MultipleHeader_C_tuple_argument ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(a=0) { P1:x:=a[1]; P2:return; };procedure Get(q=0,b=0) { G1:x:=b+q; G2:return; }; { L:call Put(<<3>>); M:call Get(3,4); E:skip; } } *)
====
