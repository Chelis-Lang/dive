---- MODULE Recursive_P_self_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
variable u=a+10;
begin
F:if a>0 then call First(a-1); else x:=u; end if; R:x:=x+u; return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
====
