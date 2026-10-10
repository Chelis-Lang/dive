---- MODULE Recursive_P_self_tail_two_params ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0,c=1)

begin
F:if a>0 then call First(a-1,c+1); return; else x:=c; end if; R:return;
end procedure;
begin
L:call First(2,3); E:skip;
end algorithm; *)
====
