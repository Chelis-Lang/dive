---- MODULE MultipleOwner_C_duplicate_slots ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(z=0,a=0) { P1:x:=a+z; P2:return; };procedure Get(z=0,a=0) { G1:x:=a+z; G2:return; }; { L:call Put(1,2); M:call Get(3,4); E:skip; } } *)
====
