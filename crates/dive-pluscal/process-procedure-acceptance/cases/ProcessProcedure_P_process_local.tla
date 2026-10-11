---- MODULE ProcessProcedure_P_process_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;

process Worker = 1
variable y=1;
begin
L:call First(y); E:y:=x;
end process;
end algorithm; *)
====
