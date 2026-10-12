---- MODULE Recursive_P_duplicate_scope ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:call Second(a); R:return;
end procedure;
procedure Second(a=1)

begin
S:call First(a); T:return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
====
