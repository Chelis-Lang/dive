---- MODULE Marker_C_unfair_first_same_line_crlf ----
EXTENDS Integers
(* --algorithm A { variable x = 0; { L: x := 1; } } --fair algorithm B { variable x = 0; { L: x := 2; } } *)
====
