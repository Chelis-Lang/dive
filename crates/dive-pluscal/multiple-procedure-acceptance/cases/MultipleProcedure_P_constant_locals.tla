---- MODULE MultipleProcedure_P_constant_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=x+a; R:return;
end procedure;
process One = 1
variable y=1;
begin
L:call First(y); E:y:=x;
end process;
process Two = 2
variable z=2;
begin
M:call First(z); N:z:=x;
end process;
end algorithm; *)
====
