---- MODULE ProcessHeader_P_boundary_fair_process ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;
fair process Worker = 1
begin
L:call First(2); E:skip;
end process;

end algorithm; *)
====
