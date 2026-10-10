---- MODULE Recursive_P_reserved_parameter ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(Next=0)

begin
F:call First(Next); R:return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
====
