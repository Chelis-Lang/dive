---- MODULE ProcessProcedure_P_member_process_local_assign ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:y:=a; R:return;
end procedure;

process Worker \in {1,2}
variable y=self;
begin
L:call First(2); E:skip;
end process;
end algorithm; *)
====
