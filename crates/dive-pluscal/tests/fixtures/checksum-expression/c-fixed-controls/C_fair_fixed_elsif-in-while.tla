---- MODULE C_fair_fixed_elsif_in_while ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables x=0,y=0; { L:while(x<2){if(x=0){x:=1;}else{if(x=1){x:=2;}}}; E:skip; } } *)
====
