---- MODULE MultipleProcedure_P_no_parameters ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First()

begin
F:x:=x+1; R:return;
end procedure;
process One = 1

begin
L:call First(); E:skip;
end process;
process Two = 2

begin
M:call First(); N:skip;
end process;
end algorithm; *)
====
