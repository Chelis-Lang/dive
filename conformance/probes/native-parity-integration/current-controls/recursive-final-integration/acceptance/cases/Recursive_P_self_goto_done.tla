---- MODULE Recursive_P_self_goto_done ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:call First(a-1); goto Done;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
====
