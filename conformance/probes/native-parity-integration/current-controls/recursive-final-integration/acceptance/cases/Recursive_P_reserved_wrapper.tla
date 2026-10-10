---- MODULE Recursive_P_reserved_wrapper ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Next(a=0)

begin
F:call Next(a); R:return;
end procedure;
begin
L:call Next(2); E:skip;
end algorithm; *)
====
