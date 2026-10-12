---- MODULE Procedure_P_proc_label_collision ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
Put: x:=a; return;
end procedure;
begin
L: call Put(2); E: skip;
end algorithm; *)
====
