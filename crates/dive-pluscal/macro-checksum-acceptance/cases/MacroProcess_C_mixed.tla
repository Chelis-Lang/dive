---- MODULE MacroProcess_C_mixed ----
EXTENDS Integers, TLC
(* --algorithm A { variable x=0,y=0; macro Put(v,a) { v:=a; }; process (P=1) variable z=0; { L: Put(z,self); E: skip; }process (Q\in {2,3}) variable w=0; { M: Put(w,self); F: skip; } } *)
====
