---- MODULE MultipleProcedure_P_procedure_process_name ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure One(a=0)

begin
F:x:=x+a; R:return;
end procedure;
process One = 1

begin
L:call One(1); E:skip;
end process;
process Two = 2

begin
M:call One(2); N:skip;
end process;
end algorithm; *)
====
