---- MODULE MultipleHeader_C_params_reversed_first ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(a=0,z=0) { P1:x:=a+z; P2:return; };procedure Get(q=0,b=0) { G1:x:=b+q; G2:return; }; { L:call Put(1,2); M:call Get(3,4); E:skip; } } *)
====
