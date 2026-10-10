---- MODULE Recursive_P_self_either ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:if a>0 then either call First(a-1); or x:=a; end either; else x:=0; end if; R:return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
====
