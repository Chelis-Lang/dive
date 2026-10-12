---- MODULE ProcessProcedure_P_member_empty ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;

process Worker \in {}

begin
L:call First(2); E:skip;
end process;
end algorithm; *)
====
