---- MODULE ProcessProcedure_P_tail_parallel_args ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0,c=1)

begin
F:if a>0 then call First(a-1,a); return; else x:=c; end if; R:return;
end procedure;

process Worker = 1

begin
L:call First(2,3); E:skip;
end process;
end algorithm; *)
====
