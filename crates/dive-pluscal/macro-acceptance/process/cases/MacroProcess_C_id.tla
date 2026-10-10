---- MODULE MacroProcess_C_id ----
EXTENDS Integers, TLC
(* --algorithm A { variable x=0,y=0; macro Put(v,a) { v:=a; }; process (P=1) variable z=0; { L: Put(z,self); E: skip; } } *)
====
