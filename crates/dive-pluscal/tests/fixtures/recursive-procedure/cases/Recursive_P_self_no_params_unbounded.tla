---- MODULE Recursive_P_self_no_params_unbounded ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First()

begin
F:call First(); R:return;
end procedure;
begin
L:call First(); E:skip;
end algorithm; *)
====
