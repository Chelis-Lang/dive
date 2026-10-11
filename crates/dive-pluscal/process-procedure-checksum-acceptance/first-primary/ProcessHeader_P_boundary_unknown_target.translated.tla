---- MODULE ProcessHeader_P_boundary_unknown_target ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;
process Worker = 1
begin
L:call Missing(2); E:skip;
end process;

end algorithm; *)
====
