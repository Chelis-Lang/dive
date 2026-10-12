---- MODULE Recursive_P_self_tail_changed_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
variable u=0;
begin
F:u:=a; call First(a-1); return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
====
