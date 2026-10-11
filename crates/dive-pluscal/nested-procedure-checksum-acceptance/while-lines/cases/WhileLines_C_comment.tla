---- MODULE WhileLines_C_comment ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(z=0,a=0) { P:x:=z+a; R:return; }; { L:while (( (* line
break *) TRUE )) { skip; }; if (TRUE) { call Put(1,2); goto E; } else { skip; }; E:skip; } } *)
====
