---- MODULE ProcessProcedure_P_process_local_assign ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:y:=a; R:return;
end procedure;

process Worker = 1
variable y=1;
begin
L:call First(2); E:skip;
end process;
end algorithm; *)
====
