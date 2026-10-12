---- MODULE ProcessHeader_P_boundary_mixed_blocks ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;
process Worker = 1
begin
L:call First(2); E:skip;
end process;
process Other=2 begin M:call First(3); Z:skip; end process;
end algorithm; *)
====
