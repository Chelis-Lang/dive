---- MODULE MultipleProcedure_P_tail_parallel_arguments ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0,b=1)

begin
F:if a>0 then call First(a-1,a); return; else x:=x+b; end if; R:return;
end procedure;
process One = 1

begin
L:call First(1,3); E:skip;
end process;
process Two = 2

begin
M:call First(2,4); N:skip;
end process;
end algorithm; *)
====
