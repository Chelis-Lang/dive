---- MODULE MultipleProcedure_P_three_interleaved ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=x+a; R:return;
end procedure;
process One \in {3}

begin
L:call First(1); E:skip;
end process;
process Two = 2

begin
M:call First(2); N:skip;
end process;
process Three \in {1}

begin
O:call First(self); P:skip;
end process;
end algorithm; *)
====
