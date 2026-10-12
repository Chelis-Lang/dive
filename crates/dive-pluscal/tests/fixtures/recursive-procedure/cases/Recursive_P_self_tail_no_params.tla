---- MODULE Recursive_P_self_tail_no_params ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First()

begin
F:call First(); return;
end procedure;
begin
L:call First(); E:skip;
end algorithm; *)
====
