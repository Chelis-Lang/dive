---- MODULE MacroProcess_C_free_self ----
EXTENDS Integers, TLC
(* --algorithm A { variable x=0,y=0; macro Put(v,a) { v:=self; }; process (P=1) variable z=0; { L: Put(z,self); E: skip; } } *)
====
