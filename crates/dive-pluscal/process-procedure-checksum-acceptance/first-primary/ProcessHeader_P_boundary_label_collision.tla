---- MODULE ProcessHeader_P_boundary_label_collision ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
First:x:=a; R:return;
end procedure;
process Worker = 1
begin
L:call First(2); E:skip;
end process;

end algorithm; *)
====
