---- MODULE Recursive_P_false_cycle ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:if FALSE then call First(a); else x:=a; end if; R:return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
====
