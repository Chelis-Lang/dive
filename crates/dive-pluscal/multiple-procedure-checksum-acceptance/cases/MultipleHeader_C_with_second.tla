---- MODULE MultipleHeader_C_with_second ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(z=0,a=0) { P1:x:=a+z; P2:return; };procedure Get(q=0,b=0) { G1:x:=b+q; G2:return; }; { L:with (t=2) { call Get(t,1); goto E; }; E:skip; } } *)
====
