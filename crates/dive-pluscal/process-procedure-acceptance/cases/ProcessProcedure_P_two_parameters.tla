---- MODULE ProcessProcedure_P_two_parameters ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0,c=a+1)

begin
F:x:=a+c; R:return;
end procedure;

process Worker = 1

begin
L:call First(2,3); E:skip;
end process;
end algorithm; *)
====
