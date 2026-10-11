---- MODULE MacroFull ----
EXTENDS Integers
(* --algorithm A
variable x = 0;
macro Set(v, a) begin v := a; end macro;
begin Start: Set(x, 1 + 2); Finish: skip; end algorithm; *)
====
