---- MODULE ProcessProcedure_P_member_proc_self ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=self; R:return;
end procedure;

process Worker \in {1,2}

begin
L:call First(2); E:skip;
end process;
end algorithm; *)
====
