---- MODULE ProcessHeader_P_constant_ordinary ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First()

begin
F:x:=1; R:return;
end procedure;
process Worker = 1
begin
L:call First(); E:skip;
end process;

end algorithm; *)
====
