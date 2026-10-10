---- MODULE MultipleProcedure_P_default_process_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=x+a; R:return;
end procedure;
process One = 1
variable y;
begin
L:call First(1); E:skip;
end process;
process Two = 2
variable z;
begin
M:call First(2); N:skip;
end process;
end algorithm; *)
====
