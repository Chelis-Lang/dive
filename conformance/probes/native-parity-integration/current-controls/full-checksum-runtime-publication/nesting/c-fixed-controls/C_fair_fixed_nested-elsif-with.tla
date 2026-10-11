---- MODULE C_fair_fixed_nested_elsif_with ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables x=0,y=0; { L:with(a=1,b=2){if(x=0){x:=a;}else{if(x=1){x:=b;}else{skip;}}}; E:skip; } } *)
====
