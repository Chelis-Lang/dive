---- MODULE MacroFullC ----
EXTENDS Integers
(* --algorithm A {
variable x = 0;
macro Set(v, a) { v := a; };
{ Start: Set(x, 1 + 2); Finish: skip; }
} *)
====
