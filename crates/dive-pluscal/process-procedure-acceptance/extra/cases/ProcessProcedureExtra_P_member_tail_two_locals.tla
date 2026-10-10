---- MODULE ProcessProcedureExtra_P_member_tail_two_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
variable u=a+1,v=u+1;
begin
F:if a>0 then call First(a-1); return; else x:=u+v; end if; R:return;
end procedure;
process Worker \in {1,2}
begin
L:call First(2); E:skip;
end process;
end algorithm; *)
====
