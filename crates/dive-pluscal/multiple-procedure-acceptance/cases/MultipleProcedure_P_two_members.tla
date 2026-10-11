---- MODULE MultipleProcedure_P_two_members ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=x+a; R:return;
end procedure;
process One \in {1,2}

begin
L:call First(1); E:skip;
end process;
process Two \in {3,4}

begin
M:call First(2); N:skip;
end process;
end algorithm; *)
====
