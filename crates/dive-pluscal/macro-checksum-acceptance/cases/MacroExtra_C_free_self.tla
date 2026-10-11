---- MODULE MacroExtra_C_free_self ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := self; }; process (P=1) { variable z=0; L: Put(z,1); E: skip; } process (Q \in {2,3}) { variable w=0; M: Put(w,self); F: skip; } } *)
====
