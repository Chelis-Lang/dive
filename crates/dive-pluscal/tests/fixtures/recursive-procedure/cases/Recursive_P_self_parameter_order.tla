---- MODULE Recursive_P_self_parameter_order ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0,c=a+1)

begin
F:if a>0 then call First(a-1,c+1); else x:=c; end if; R:return;
end procedure;
begin
L:call First(2,3); E:skip;
end algorithm; *)
====
