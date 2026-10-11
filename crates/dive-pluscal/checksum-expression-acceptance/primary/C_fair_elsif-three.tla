---- MODULE C_fair_elsif_three ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables x=0,y=0; { L:if(x=0){x:=1;}else{if(x=1){x:=2;}else{if(x=2){skip;}else{x:=3;}}} E:skip; } } *)
====
