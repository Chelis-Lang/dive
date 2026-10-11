---- MODULE ProcessHeader_P_boundary_duplicate_global ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variables x=0,a=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;
process Worker = 1
begin
L:call First(2); E:skip;
end process;

end algorithm; *)
====
