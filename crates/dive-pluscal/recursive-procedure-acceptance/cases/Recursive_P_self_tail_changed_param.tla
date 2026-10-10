---- MODULE Recursive_P_self_tail_changed_param ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:a:=a-1; call First(a); return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
====
