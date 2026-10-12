---- MODULE Inter_C_elsif ----
EXTENDS Naturals,TLC
(* --algorithm A { variables x=0,y=0; { if (x=0) { x:=1; } elsif (x=1) { y:=1; } else { skip; }; x:=2; } } *)
====
