---- MODULE MultiBound_C_escaped_overlap ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0; process (P ="a\tb") variable x=0; { L: x:=1; E: g:=x; } process (Q ="a\tb") variable y=0; { M: y:=2; F: g:=y; } } *)
====
