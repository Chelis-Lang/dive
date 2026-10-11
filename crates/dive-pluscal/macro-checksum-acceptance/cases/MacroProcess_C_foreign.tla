---- MODULE MacroProcess_C_foreign ----
EXTENDS Integers, TLC
(* --algorithm A { variable x=0,y=0; macro Put(v,a) { v:=w; }; process (P\in {1,2}) variable z=0; { L: Put(z,self); E: skip; }process (Q\in {3,4}) variable w=0; { M: Put(w,self); F: skip; } } *)
====
