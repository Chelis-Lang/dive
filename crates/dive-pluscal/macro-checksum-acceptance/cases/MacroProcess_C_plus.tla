---- MODULE MacroProcess_C_plus ----
EXTENDS Integers, TLC
(* --algorithm A { variable x=0,y=0; macro Put(v,a) { v:=a; }; process (P\in {1,2}) variable z=0; { L: + Put(z,self); E: skip; } } *)
====
