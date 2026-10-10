---- MODULE Recursive_P_mutual_tail_unbounded ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:call Second(a+1); return;
end procedure;
procedure Second(b=0)

begin
S:call First(b+1); return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
====
