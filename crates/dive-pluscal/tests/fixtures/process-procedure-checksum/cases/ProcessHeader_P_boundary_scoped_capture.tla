---- MODULE ProcessHeader_P_boundary_scoped_capture ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;
process Worker = 1
begin
L:with a=2 do call First(a); end with; E:skip;
end process;

end algorithm; *)
====
