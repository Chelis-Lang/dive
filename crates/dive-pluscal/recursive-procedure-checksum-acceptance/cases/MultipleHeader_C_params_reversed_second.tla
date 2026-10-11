---- MODULE MultipleHeader_C_params_reversed_second ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(z=0,a=0) { P1:x:=a+z; P2:return; };procedure Get(b=0,q=0) { G1:x:=b+q; G2:return; }; { L:call Put(1,2); M:call Get(3,4); E:skip; } } *)
====
