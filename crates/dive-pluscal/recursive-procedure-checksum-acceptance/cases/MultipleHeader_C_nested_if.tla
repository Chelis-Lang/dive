---- MODULE MultipleHeader_C_nested_if ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(z=0,a=0) { P1:x:=a+z; P2:return; };procedure Get(q=0,b=0) { G1:x:=b+q; G2:return; }; { L:if (x=0) { if (x<1) { call Put(1,2); goto E; } else { call Get(3,4); goto E; }; } else { skip; }; E:skip; } } *)
====
