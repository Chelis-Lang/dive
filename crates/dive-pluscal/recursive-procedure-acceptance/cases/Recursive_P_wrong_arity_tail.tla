---- MODULE Recursive_P_wrong_arity_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:call First(a,a); return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
====
