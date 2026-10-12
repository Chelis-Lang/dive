---- MODULE Recursive_P_self_base_zero ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:if a>0 then call First(a-1); else x:=x+1; end if; R:x:=x+1; return;
end procedure;
begin
L:call First(0); E:skip;
end algorithm; *)
====
