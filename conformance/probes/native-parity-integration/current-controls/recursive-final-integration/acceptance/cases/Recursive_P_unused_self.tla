---- MODULE Recursive_P_unused_self ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;
procedure Second(b=0)

begin
S:call Second(b); T:return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
====
