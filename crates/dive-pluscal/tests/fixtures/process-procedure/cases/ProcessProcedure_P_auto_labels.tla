---- MODULE ProcessProcedure_P_auto_labels ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
x:=a; return;
end procedure;

process Worker = 1

begin
call First(2); skip;
end process;
end algorithm; *)
====
