---- MODULE ProcessProcedure_P_member_process_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;

process Worker \in {1,2}
variable y=self;
begin
L:call First(y); E:y:=x;
end process;
end algorithm; *)
====
