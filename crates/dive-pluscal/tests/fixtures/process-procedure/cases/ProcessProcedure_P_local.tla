---- MODULE ProcessProcedure_P_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
variable u=a+1;
begin
F:x:=u; R:return;
end procedure;

process Worker = 1

begin
L:call First(2); E:skip;
end process;
end algorithm; *)
====
