---- MODULE ProcessProcedureNames_P_wrapper ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure ProcSet(a=0)

begin
F:x:=a; R:return;
end procedure;

process Worker = 1

begin
L:call ProcSet(2); E:skip;
end process;
end algorithm; *)
====
