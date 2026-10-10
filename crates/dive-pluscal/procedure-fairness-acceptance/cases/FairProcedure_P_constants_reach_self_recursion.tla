---- MODULE FairProcedure_P_constants_reach_self_recursion ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:if a>0 then call First(a-1); else x:=x+1; end if; R:return;
end procedure;
fair process One = 1

begin
L:call First(1); E:skip;
end process;
fair + process Two = 2

begin
M:call First(2); N:skip;
end process;
end algorithm; *)
====
