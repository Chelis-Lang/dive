---- MODULE ProcessProcedure_P_self_recursive_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
variable u=a+1;
begin
F:if a>0 then call First(a-1); else x:=u; end if; R:x:=x+u; return;
end procedure;

process Worker = 1

begin
L:call First(2); E:skip;
end process;
end algorithm; *)
====
