---- MODULE MultipleHeader_C_locals_outside_body ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(z=0,a=0) variable u=0; { P1:u:=a+z; P2:x:=u; P3:return; };procedure Get(q=0,b=0) variable v=0; { G1:v:=b+q; G2:x:=v; G3:return; }; { L:call Put(1,2); M:call Get(3,4); E:skip; } } *)
====
