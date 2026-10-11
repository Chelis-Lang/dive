---- MODULE MultipleProcedure_P_tail_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
variable u=a+1,v=u+1;
begin
F:if a>0 then call First(a-1); return; else x:=x+u+v; end if; R:return;
end procedure;
process One = 1

begin
L:call First(1); E:skip;
end process;
process Two = 2

begin
M:call First(2); N:skip;
end process;
end algorithm; *)
====
