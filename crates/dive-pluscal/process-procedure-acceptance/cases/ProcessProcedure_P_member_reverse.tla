---- MODULE ProcessProcedure_P_member_reverse ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;

process Worker \in 2..1

begin
L:call First(2); E:skip;
end process;
end algorithm; *)
====
