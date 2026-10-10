---- MODULE ProcessProcedureExtra_P_pc_indexed ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=pc[self]; R:return;
end procedure;
process Worker \in {1,2}
begin
L:call First(2); E:skip;
end process;
end algorithm; *)
====
