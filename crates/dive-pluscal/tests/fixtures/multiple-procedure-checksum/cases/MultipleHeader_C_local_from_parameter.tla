---- MODULE MultipleHeader_C_local_from_parameter ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(z=0,a=0) { variable u=a;P1:x:=u+z; P2:return; };procedure Get(q=0,b=0) { variable v=b;G1:x:=v+q; G2:return; }; { L:call Put(1,2); M:call Get(3,4); E:skip; } } *)
====
