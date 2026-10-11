---- MODULE ProcessHeader_P_constant_unused ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;
process Worker = 1
begin
L:x:=1; E:skip;
end process;

end algorithm; *)
====
